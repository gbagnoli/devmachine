use super::*;
use crate::credential_cache::CacheError;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Mutex,
    },
};

#[derive(Default)]
struct FakeCache {
    values: Mutex<BTreeMap<Vec<u8>, Value>>,
    now: AtomicU64,
    stores: AtomicUsize,
    forgets: AtomicUsize,
    deny_expiry: AtomicBool,
}
struct Value {
    text: String,
    until: u64,
}
impl CredentialCache for FakeCache {
    fn read(&self, scope: &[u8]) -> Result<Option<Zeroizing<String>>, CacheError> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .get(scope)
            .filter(|value| value.until > self.now.load(Ordering::SeqCst))
            .map(|value| Zeroizing::new(value.text.clone())))
    }
    fn remember_verified(
        &self,
        scope: &[u8],
        value: &str,
        lifetime: Duration,
    ) -> Result<(), CacheError> {
        if self.deny_expiry.load(Ordering::SeqCst) {
            return Err(CacheError::Expiry);
        }
        self.stores.fetch_add(1, Ordering::SeqCst);
        self.values.lock().unwrap().insert(
            scope.to_vec(),
            Value {
                text: value.into(),
                until: self.now.load(Ordering::SeqCst) + lifetime.as_secs(),
            },
        );
        Ok(())
    }
    fn forget(&self, scope: &[u8]) -> Result<(), CacheError> {
        self.forgets.fetch_add(1, Ordering::SeqCst);
        self.values.lock().unwrap().remove(scope);
        Ok(())
    }
    fn clear(&self) -> Result<(), CacheError> {
        self.values.lock().unwrap().clear();
        Ok(())
    }
}
fn prompt(value: &'static str) -> PasswordPrompt {
    Arc::new(move || Ok(Zeroizing::new(value.into())))
}

#[test]
fn only_successful_authentication_persists_and_later_consumers_do_not_refresh_expiry() {
    let cache = Arc::new(FakeCache::default());
    let binding = CacheBinding::new(cache.clone(), [1; 32]);
    let mut first = PasswordState::default();
    assert_eq!(
        first
            .offer(1, Some(&prompt("first")), Some(&binding))
            .unwrap(),
        "first"
    );
    assert!(cache.read(&[1; 32]).unwrap().is_none());
    first.complete(1, true, Some(&binding));
    drop(first);
    assert_eq!(cache.stores.load(Ordering::SeqCst), 1);
    let mut later = PasswordState::default();
    assert_eq!(later.offer(1, None, Some(&binding)).unwrap(), "first");
    later.sent();
    later.complete(1, true, Some(&binding));
    assert_eq!(cache.stores.load(Ordering::SeqCst), 1);
    cache.now.store(1801, Ordering::SeqCst);
    assert!(later.offer(2, None, Some(&binding)).is_err());
    assert_eq!(
        later
            .offer(3, Some(&prompt("new")), Some(&binding))
            .unwrap(),
        "new"
    );
    later.complete(3, true, Some(&binding));
    assert_eq!(cache.stores.load(Ordering::SeqCst), 2);
}

#[test]
fn rejected_and_unverified_answers_are_forgotten_without_retrying_commands() {
    let cache = Arc::new(FakeCache::default());
    let binding = CacheBinding::new(cache.clone(), [2; 32]);
    cache
        .remember_verified(&[2; 32], "old", SUDO_CACHE_LIFETIME)
        .unwrap();
    let mut state = PasswordState::default();
    assert_eq!(state.offer(1, None, Some(&binding)).unwrap(), "old");
    state.sent();
    assert_eq!(
        state
            .offer(1, Some(&prompt("replacement")), Some(&binding))
            .unwrap(),
        "replacement"
    );
    assert!(cache.read(&[2; 32]).unwrap().is_none());
    state.complete(1, false, Some(&binding));
    assert!(state.value.is_none());
    assert!(cache.read(&[2; 32]).unwrap().is_none());
    state.complete(2, true, Some(&binding)); // no password was requested
    assert_eq!(cache.stores.load(Ordering::SeqCst), 1);
}

#[test]
fn unavailable_kernel_falls_back_to_bounded_memory_and_lock_revokes_next_use() {
    let cache = Arc::new(FakeCache::default());
    cache.deny_expiry.store(true, Ordering::SeqCst);
    let binding = CacheBinding::new(cache.clone(), [3; 32]);
    let mut state = PasswordState::default();
    assert_eq!(
        state
            .offer(1, Some(&prompt("fallback")), Some(&binding))
            .unwrap(),
        "fallback"
    );
    state.complete(1, true, Some(&binding));
    assert!(matches!(state.origin, Origin::Memory(_)));
    assert!(cache.read(&[3; 32]).unwrap().is_none());
    assert_eq!(state.offer(2, None, Some(&binding)).unwrap(), "fallback");
    state.origin = Origin::Memory(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    assert!(state.offer(3, None, Some(&binding)).is_err());
    cache.deny_expiry.store(false, Ordering::SeqCst);
    state
        .offer(4, Some(&prompt("persistent")), Some(&binding))
        .unwrap();
    state.complete(4, true, Some(&binding));
    cache.clear().unwrap();
    assert!(state.offer(5, None, Some(&binding)).is_err());
}
