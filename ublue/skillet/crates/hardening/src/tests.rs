use super::*;
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

struct MockSystem {
    groups: Arc<Mutex<HashSet<String>>>,
}

impl MockSystem {
    fn new() -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl SystemResource for MockSystem {
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

struct MockFiles {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockFiles {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl FileResource for MockFiles {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        _mode: Option<u32>,
        _owner: Option<&str>,
        _group: Option<&str>,
    ) -> Result<bool, FileError> {
        let mut files = self.files.lock().unwrap();
        let path_str = path.display().to_string();
        if let Some(existing) = files.get(&path_str) {
            if existing == content {
                return Ok(false);
            }
        }
        files.insert(path_str, content.to_vec());
        Ok(true)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        let mut files = self.files.lock().unwrap();
        Ok(files.remove(&path.display().to_string()).is_some())
    }
}

#[test]
fn test_hardening_applies_sysctl() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    assert!(files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/sysctl.d/99-hardening.conf"));
}
