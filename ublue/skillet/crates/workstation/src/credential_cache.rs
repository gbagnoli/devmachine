//! User-local, purpose-separated kernel credentials with mandatory fixed expiry.
use keyutils::{keytypes::User, Keyring, Permission, SpecialKeyring};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, sync::Mutex, time::Duration};
use zeroize::Zeroizing;

static RINGS: Mutex<BTreeMap<String, Keyring>> = Mutex::new(BTreeMap::new());

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("kernel credential cache unavailable")]
    Kernel(#[from] keyutils::Error),
    #[error("invalid credential cache namespace, lifetime or payload")]
    Invalid,
    #[error("credential cache registry lock poisoned")]
    Poisoned,
    #[error("kernel denied credential expiry; entry removed or revoked")]
    Expiry,
    #[error("kernel denied expiry and cache entry cleanup")]
    Cleanup,
}

/// Consumers supply a context binding and store only successfully authenticated values.
/// Reading or remembering an already matching value must not renew its lifetime.
pub trait CredentialCache: Send + Sync {
    fn read(&self, scope: &[u8]) -> Result<Option<Zeroizing<String>>, CacheError>;
    fn remember_verified(
        &self,
        scope: &[u8],
        value: &str,
        lifetime: Duration,
    ) -> Result<(), CacheError>;
    fn forget(&self, scope: &[u8]) -> Result<(), CacheError>;
    fn clear(&self) -> Result<(), CacheError>;
}

pub struct KernelCache {
    namespace: String,
}
impl KernelCache {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
    fn description(&self, scope: &[u8]) -> String {
        format!(
            "skillet:{}:{}",
            self.namespace,
            hex::encode(Sha256::digest(scope))
        )
    }
    fn ring(&self) -> Result<Keyring, CacheError> {
        if self.namespace.is_empty()
            || self.namespace.len() > 64
            || !self
                .namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(CacheError::Invalid);
        }
        let name = format!("skillet-{}-cache", self.namespace);
        let mut rings = RINGS.lock().map_err(|_| CacheError::Poisoned)?;
        if let Some(ring) = rings.get(&name) {
            return Ok(ring.clone());
        }
        let current = Keyring::attach_or_create(SpecialKeyring::Session)
            .ok()
            .and_then(|ring| {
                ring.description()
                    .ok()
                    .filter(|description| description.description == name)
                    .map(|_| ring)
            });
        let mut ring = match current {
            Some(ring) => ring,
            None => Keyring::join_session(&name)?,
        };
        ring.set_permissions(Permission::POSSESSOR_ALL | Permission::USER_ALL)?;
        let mut user = Keyring::attach_or_create(SpecialKeyring::User)?;
        user.link_keyring(&ring)?;
        rings.insert(name, ring.clone());
        Ok(ring)
    }
}

fn absent(error: keyutils::Error) -> bool {
    matches!(
        nix::errno::Errno::from_raw(error.0),
        nix::errno::Errno::ENOKEY | nix::errno::Errno::EKEYEXPIRED | nix::errno::Errno::EKEYREVOKED
    )
}
fn valid_lifetime(lifetime: Duration) -> bool {
    lifetime.as_secs() > 0
        && lifetime.subsec_nanos() == 0
        && u32::try_from(lifetime.as_secs()).is_ok()
}
impl CredentialCache for KernelCache {
    fn read(&self, scope: &[u8]) -> Result<Option<Zeroizing<String>>, CacheError> {
        let ring = self.ring()?;
        let key = match ring.search_for_key::<User, _, _>(self.description(scope), None) {
            Ok(key) => key,
            Err(error) if absent(error) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let bytes = match key.read() {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(error) if absent(error) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        std::str::from_utf8(&bytes)
            .map(|value| Some(Zeroizing::new(value.to_owned())))
            .map_err(|_| CacheError::Invalid)
    }
    fn remember_verified(
        &self,
        scope: &[u8],
        value: &str,
        lifetime: Duration,
    ) -> Result<(), CacheError> {
        if !valid_lifetime(lifetime) {
            return Err(CacheError::Invalid);
        }
        if self
            .read(scope)?
            .as_deref()
            .is_some_and(|cached| cached == value)
        {
            return Ok(());
        }
        let mut ring = self.ring()?;
        let mut key = ring.add_key::<User, _, _>(self.description(scope), value.as_bytes())?;
        if key
            .set_permissions(Permission::POSSESSOR_ALL | Permission::USER_ALL)
            .and_then(|()| key.set_timeout(lifetime))
            .is_err()
        {
            let removed = ring.unlink_key(&key).is_ok();
            let revoked = key.revoke().is_ok();
            return Err(if removed || revoked {
                CacheError::Expiry
            } else {
                CacheError::Cleanup
            });
        }
        Ok(())
    }
    fn forget(&self, scope: &[u8]) -> Result<(), CacheError> {
        let mut ring = self.ring()?;
        match ring.search_for_key::<User, _, _>(self.description(scope), None) {
            Ok(key) => {
                ring.unlink_key(&key)?;
                let _ = key.revoke();
            }
            Err(error) if absent(error) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
    fn clear(&self) -> Result<(), CacheError> {
        let mut ring = self.ring()?;
        // keyutils inspects every child; an expired child can make enumeration
        // fail even though clearing the ring itself remains valid.
        let keys = match ring.read() {
            Ok((keys, _)) => keys,
            Err(error) if absent(error) => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        ring.clear()?;
        for key in keys {
            let _ = key.revoke();
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "credential_cache/tests.rs"]
mod tests;
