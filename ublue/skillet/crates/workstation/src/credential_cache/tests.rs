use super::*;

#[test]
fn purposes_and_contexts_are_separate_without_disclosing_context_or_values() {
    let vault = KernelCache::new("vault");
    let sudo = KernelCache::new("sudo");
    assert_ne!(vault.description(b"context"), sudo.description(b"context"));
    assert_ne!(
        sudo.description(b"host-a:user"),
        sudo.description(b"host-b:user")
    );
    assert!(!sudo
        .description(b"private-host:user")
        .contains("private-host"));
    assert_eq!(
        vault.description(b"path"),
        format!("skillet:vault:{}", hex::encode(Sha256::digest(b"path")))
    );
}

#[test]
fn expiry_cannot_mean_forever_or_overflow_the_kernel_seconds_field() {
    assert!(!valid_lifetime(Duration::ZERO));
    assert!(!valid_lifetime(Duration::from_millis(500)));
    assert!(!valid_lifetime(Duration::from_secs(
        u64::from(u32::MAX) + 1
    )));
    assert!(valid_lifetime(Duration::from_mins(30)));
    assert!(valid_lifetime(Duration::from_hours(3)));
}

#[test]
#[ignore = "requires a Linux kernel allowing user keyrings and expiry"]
fn real_kernel_cache_child() {
    let Ok(namespace) = std::env::var("SKILLET_TEST_CACHE_NAMESPACE") else {
        return;
    };
    assert_eq!(
        KernelCache::new(namespace)
            .read(b"probe")
            .unwrap()
            .as_deref()
            .map(String::as_str),
        Some("synthetic-cache-value")
    );
}

struct Cleanup<'a>(&'a KernelCache);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = self.0.clear();
        if let Ok(ring) = self.0.ring() {
            if let Ok(mut user) = Keyring::attach_or_create(SpecialKeyring::User) {
                let _ = user.unlink_keyring(&ring);
            }
        }
    }
}

#[test]
#[ignore = "requires a Linux kernel allowing user keyrings and expiry"]
fn real_kernel_cache_cross_process_fixed_expiry_and_namespace_isolation() {
    let namespace = format!(
        "probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let cache = KernelCache::new(&namespace);
    let other = KernelCache::new(format!("{namespace}-other"));
    let _cleanup = Cleanup(&cache);
    let _other_cleanup = Cleanup(&other);
    cache
        .remember_verified(b"probe", "synthetic-cache-value", Duration::from_secs(6))
        .unwrap();
    other
        .remember_verified(b"probe", "independent-value", Duration::from_secs(30))
        .unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "credential_cache::tests::real_kernel_cache_child",
            "--ignored",
            "--exact",
        ])
        .env("SKILLET_TEST_CACHE_NAMESPACE", &namespace)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "child did not read the kernel credential cache"
    );
    std::thread::sleep(Duration::from_secs(2));
    assert!(cache.read(b"probe").unwrap().is_some());
    // A cache hit must not reset the original six-second expiry to thirty minutes.
    cache
        .remember_verified(b"probe", "synthetic-cache-value", Duration::from_mins(30))
        .unwrap();
    std::thread::sleep(Duration::from_millis(4500));
    assert!(cache.read(b"probe").unwrap().is_none());
    cache.clear().unwrap();
    assert_eq!(
        other.read(b"probe").unwrap().as_deref().map(String::as_str),
        Some("independent-value")
    );
}
