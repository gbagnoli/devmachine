use anyhow::{Context, Result};
use keyutils::{keytypes::User, Keyring, Permission, SpecialKeyring};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex, time::Duration};

const CACHE_LIFETIME: Duration = Duration::from_hours(3);
const SESSION_KEYRING_NAME: &str = "skillet-vault-cache";

static SESSION_KEYRING: Mutex<Option<Keyring>> = Mutex::new(None);

fn description(database: &Path) -> String {
    let digest = Sha256::digest(database.as_os_str().as_encoded_bytes());
    format!("skillet:vault:{}", hex::encode(digest))
}

fn keyring() -> Result<Keyring> {
    // Keys in the user keyring are not possessed by this process on every
    // workstation, so setting their expiry can be denied. A named session
    // keyring gives the process possession permissions. Link it from the user
    // keyring so it remains available to later Skillet invocations after this
    // process exits.
    let mut cached = SESSION_KEYRING
        .lock()
        .map_err(|_| anyhow::anyhow!("vault session keyring lock was poisoned"))?;
    if let Some(ring) = cached.as_ref() {
        return Ok(ring.clone());
    }

    let current = Keyring::attach_or_create(SpecialKeyring::Session)
        .ok()
        .and_then(|ring| {
            ring.description()
                .ok()
                .filter(|description| description.description == SESSION_KEYRING_NAME)
                .map(|_| ring)
        });
    let mut ring = match current {
        Some(ring) => ring,
        None => Keyring::join_session(SESSION_KEYRING_NAME)
            .context("opening the named Linux session keyring for the vault cache")?,
    };
    // Joining an existing named session keyring requires owner search access.
    // The user keyring link below keeps the named ring alive between CLI runs.
    ring.set_permissions(Permission::POSSESSOR_ALL | Permission::USER_SEARCH)
        .context("setting access on the named vault session keyring")?;
    let mut user = Keyring::attach_or_create(SpecialKeyring::User)
        .context("opening the Linux user keyring for the vault cache")?;
    user.link_keyring(&ring)
        .context("keeping the vault session keyring available across invocations")?;
    *cached = Some(ring.clone());
    Ok(ring)
}

pub(super) fn read(database: &Path) -> Result<Option<String>> {
    let Ok(ring) = keyring() else {
        return Ok(None);
    };
    let Ok(key) = ring.search_for_key::<User, _, _>(description(database), None) else {
        return Ok(None);
    };
    let payload = key.read().context("reading the cached vault password")?;
    String::from_utf8(payload)
        .context("cached vault password is not UTF-8")
        .map(Some)
}

pub(super) fn store(database: &Path, password: &str) -> Result<()> {
    let mut ring = match keyring() {
        Ok(ring) => ring,
        Err(error) => {
            eprintln!(
                "Vault password was not cached: Linux session keyring unavailable: {error:#}"
            );
            return Ok(());
        }
    };
    let Ok(mut key) = ring.add_key::<User, _, _>(description(database), password.as_bytes()) else {
        eprintln!("Vault password was not cached: could not add a kernel key");
        return Ok(());
    };
    if key.set_timeout(CACHE_LIFETIME).is_err() {
        // An unbounded cache entry would retain the master password beyond the
        // requested lifetime. Drop its only link before proceeding uncached.
        ring.unlink_key(&key)
            .context("removing a vault key whose expiry could not be set")?;
        eprintln!("Vault password was not cached: kernel denied the three-hour expiry");
    }
    Ok(())
}

pub(super) fn clear(database: &Path) -> Result<()> {
    let mut ring = keyring()?;
    if let Ok(key) = ring.search_for_key::<User, _, _>(description(database), None) {
        ring.unlink_key(&key)
            .context("removing the cached vault password")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clear, read, store};

    #[test]
    fn named_session_cache_expires_and_clears() {
        let database = tempfile::tempdir().expect("temporary database path");
        let database = database.path().join("secrets.kdbx");

        store(&database, "dummy-vault-password").expect("cache password");
        assert_eq!(
            read(&database).expect("read cache").as_deref(),
            Some("dummy-vault-password")
        );

        clear(&database).expect("clear cache");
        assert_eq!(read(&database).expect("read cleared cache"), None);
    }
}
