use super::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// Mock implementation for testing consumers
pub struct MockSystemResource {
    pub groups: Arc<Mutex<HashSet<String>>>,
}

impl MockSystemResource {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl SystemResource for MockSystemResource {
    fn ensure_group(&self, name: &str) -> Result<bool, SystemError> {
        let mut groups = self.groups.lock().unwrap();
        if groups.contains(name) {
            Ok(false)
        } else {
            groups.insert(name.to_string());
            Ok(true)
        }
    }
}

#[test]
fn test_mock_system_resource() {
    let system = MockSystemResource::new();
    let changed = system.ensure_group("syslog").unwrap();
    assert!(changed);
    assert!(system.groups.lock().unwrap().contains("syslog"));

    let changed_again = system.ensure_group("syslog").unwrap();
    assert!(!changed_again);
}
