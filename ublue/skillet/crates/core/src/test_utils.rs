use crate::files::{FileError, FileResource};
use crate::system::{SystemError, SystemResource};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct MockSystem {
    pub groups: Arc<Mutex<HashSet<String>>>,
}

impl MockSystem {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl Default for MockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemResource for MockSystem {
    fn ensure_group(&self, name: &str) -> Result<bool, SystemError> {
        let mut groups = self.groups.lock().unwrap_or_else(|e| e.into_inner());
        if groups.contains(name) {
            Ok(false)
        } else {
            groups.insert(name.to_string());
            Ok(true)
        }
    }
}

pub type FileMetadata = (Option<u32>, Option<String>, Option<String>);

pub struct MockFiles {
    pub files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub metadata: Arc<Mutex<HashMap<String, FileMetadata>>>,
}

impl MockFiles {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            metadata: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl FileResource for MockFiles {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        let mut metadata = self.metadata.lock().unwrap_or_else(|e| e.into_inner());

        let mut changed = false;

        if let Some(existing) = files.get(&path_str) {
            if existing != content {
                files.insert(path_str.clone(), content.to_vec());
                changed = true;
            }
        } else {
            files.insert(path_str.clone(), content.to_vec());
            changed = true;
        }

        let new_meta = (
            mode,
            owner.map(|s| s.to_string()),
            group.map(|s| s.to_string()),
        );
        if let Some(existing_meta) = metadata.get(&path_str) {
            if existing_meta != &new_meta {
                metadata.insert(path_str, new_meta);
                changed = true;
            }
        } else {
            metadata.insert(path_str, new_meta);
            changed = true;
        }

        Ok(changed)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        let mut metadata = self.metadata.lock().unwrap_or_else(|e| e.into_inner());

        let f_removed = files.remove(&path_str).is_some();
        let m_removed = metadata.remove(&path_str).is_some();

        Ok(f_removed || m_removed)
    }
}
