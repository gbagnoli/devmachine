use anyhow::{Context, Result};
use keyutils::{keytypes::User, Keyring, SpecialKeyring};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

const CACHE_LIFETIME: Duration = Duration::from_hours(3);

fn description(database: &Path) -> String {
    let digest = Sha256::digest(database.as_os_str().as_encoded_bytes());
    format!("skillet:vault:{}", hex::encode(digest))
}

fn keyring() -> Result<Keyring> {
    // The per-user keyring survives separate CLI processes. Each password key
    // has its own timeout and can be removed without affecting other keys.
    Keyring::attach_or_create(SpecialKeyring::User)
        .context("opening the Linux user keyring for the vault cache")
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
    let Ok(mut ring) = keyring() else {
        eprintln!("Vault password was not cached: Linux user keyring unavailable");
        return Ok(());
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
