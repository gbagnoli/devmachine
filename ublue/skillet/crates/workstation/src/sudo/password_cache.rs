//! Authentication proof and fixed-expiry policy shared by every sudo consumer.
use super::{PasswordPrompt, SUDO_CACHE_LIFETIME};
use crate::credential_cache::CredentialCache;
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone)]
pub(super) struct CacheBinding {
    cache: Arc<dyn CredentialCache>,
    scope: [u8; 32],
}
impl CacheBinding {
    pub(super) fn new(cache: Arc<dyn CredentialCache>, scope: [u8; 32]) -> Self {
        Self { cache, scope }
    }
    pub(super) fn read(&self) -> Option<Zeroizing<String>> {
        self.cache.read(&self.scope).ok().flatten()
    }
    fn forget(&self) {
        if let Err(error) = self.cache.forget(&self.scope) {
            eprintln!("Could not clear scoped sudo credential cache: {error}");
        }
    }
}

enum Origin {
    None,
    Pending(Instant),
    Persistent,
    Memory(Instant),
}
pub(super) struct PasswordState {
    value: Option<Zeroizing<String>>,
    origin: Origin,
    last_operation: Option<u64>,
    used_operation: Option<u64>,
}
impl Default for PasswordState {
    fn default() -> Self {
        Self {
            value: None,
            origin: Origin::None,
            last_operation: None,
            used_operation: None,
        }
    }
}
impl PasswordState {
    pub(super) fn expire(&mut self) {
        if matches!(self.origin, Origin::Pending(until) | Origin::Memory(until) if Instant::now() >= until)
        {
            self.value = None;
            self.origin = Origin::None;
        }
    }
    pub(super) fn offer(
        &mut self,
        operation: u64,
        prompt: Option<&PasswordPrompt>,
        binding: Option<&CacheBinding>,
    ) -> io::Result<&str> {
        self.expire();
        if self.last_operation == Some(operation) {
            // sudo requested another answer: the preceding credential was rejected.
            if let Some(binding) = binding {
                binding.forget();
            }
            self.value = None;
            self.origin = Origin::None;
        } else if !matches!(self.origin, Origin::Memory(_)) {
            // Re-read the kernel for each operation; an in-memory copy must not
            // outlive kernel expiry or an explicit cache lock in another process.
            self.value = binding.and_then(CacheBinding::read);
            self.origin = if self.value.is_some() {
                Origin::Persistent
            } else {
                Origin::None
            };
        }
        if self.value.is_none() {
            let prompt =
                prompt.ok_or_else(|| io::Error::other("sudo requires an interactive password"))?;
            self.value = Some(prompt()?);
            self.origin = Origin::Pending(Instant::now() + SUDO_CACHE_LIFETIME);
        }
        let value = self
            .value
            .as_ref()
            .ok_or_else(|| io::Error::other("sudo password unavailable"))?;
        if value.len() > 4095 || value.contains(['\n', '\r', '\0']) {
            if let Some(binding) = binding {
                binding.forget();
            }
            self.value = None;
            self.origin = Origin::None;
            return Err(io::Error::other("invalid sudo password input"));
        }
        self.last_operation = Some(operation);
        self.used_operation = Some(operation);
        self.value
            .as_deref()
            .map(String::as_str)
            .ok_or_else(|| io::Error::other("sudo password unavailable"))
    }
    pub(super) fn sent(&mut self) {
        if matches!(self.origin, Origin::Persistent) {
            self.value = None;
        }
    }
    pub(super) fn complete(
        &mut self,
        operation: u64,
        success: bool,
        binding: Option<&CacheBinding>,
    ) {
        self.expire();
        if self.used_operation != Some(operation) {
            return;
        }
        if !success {
            // A failed program cannot prove authentication succeeded. Never
            // persist an unverified answer or retry a possibly executed command.
            if let Some(binding) = binding {
                binding.forget();
            }
            self.value = None;
            self.origin = Origin::None;
            return;
        }
        let Origin::Pending(until) = self.origin else {
            return;
        };
        let Some(value) = &self.value else {
            return;
        };
        let remaining =
            Duration::from_secs(until.saturating_duration_since(Instant::now()).as_secs());
        if remaining.is_zero() {
            self.value = None;
            self.origin = Origin::None;
            return;
        }
        if let Some(binding) = binding {
            match binding
                .cache
                .remember_verified(&binding.scope, value, remaining)
            {
                Ok(()) => {
                    self.value = None;
                    self.origin = Origin::Persistent;
                    return;
                }
                Err(error) => eprintln!("Sudo password was not cached across invocations: {error}"),
            }
        }
        self.origin = Origin::Memory(until);
    }
}

#[cfg(test)]
#[path = "password_cache/tests.rs"]
mod tests;
