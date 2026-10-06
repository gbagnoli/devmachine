//! Shared atomic persistence of child tokens used by distinct consumers.
use crate::{
    cloudflare::{CloudflareError, IssuedToken},
    vault::{SecretStore, VaultError},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Cloudflare(#[from] CloudflareError),
    #[error("saving the issued token and revoking it both failed; retry named-token recovery")]
    Recovery,
}

/// Issuance recovers an orphan by its deterministic consumer-specific name.
/// If vault persistence fails, revoke the new token before returning.
pub fn use_or_create(
    store: &mut impl SecretStore,
    path: &str,
    issue: impl FnOnce() -> Result<IssuedToken, CloudflareError>,
    revoke: impl FnOnce(&str) -> Result<(), CloudflareError>,
) -> Result<String, TokenError> {
    store.ensure_unchanged()?;
    if let Some(token) = store.get(path)? {
        return Ok(token);
    }
    let issued = issue()?;
    if let Err(error) = store.save_verified(path, &issued.value) {
        return match revoke(&issued.id) {
            Ok(()) => Err(error.into()),
            Err(_) => Err(TokenError::Recovery),
        };
    }
    Ok(issued.value)
}
