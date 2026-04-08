use crate::files::{FileError, FileResource};
use crate::system::{SystemError, SystemResource};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct MockSystem {
    pub groups: Arc<Mutex<HashSet<String>>>,
    pub users: Arc<Mutex<HashSet<String>>>,
    pub services: Arc<Mutex<HashMap<String, String>>>, // name -> state (started, stopped, restarted)
}

impl MockSystem {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashSet::new())),
            users: Arc::new(Mutex::new(HashSet::new())),
            services: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemResource for MockSystem {
    fn ensure_group(&self, name: &str, _gid: Option<u32>) -> Result<bool, SystemError> {
        let mut groups = self.groups.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if groups.contains(name) {
            Ok(false)
        } else {
            groups.insert(name.to_string());
            Ok(true)
        }
    }

    fn ensure_user(
        &self,
        name: &str,
        _uid: Option<u32>,
        _gid: Option<u32>,
    ) -> Result<bool, SystemError> {
        let mut users = self.users.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if users.contains(name) {
            Ok(false)
        } else {
            users.insert(name.to_string());
            Ok(true)
        }
    }

    fn service_start(&self, name: &str) -> Result<(), SystemError> {
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string(), "started".to_string());
        Ok(())
    }

    fn service_stop(&self, name: &str) -> Result<(), SystemError> {
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string(), "stopped".to_string());
        Ok(())
    }

    fn service_restart(&self, name: &str) -> Result<(), SystemError> {
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string(), "restarted".to_string());
        Ok(())
    }

    fn service_reload(&self, name: &str) -> Result<(), SystemError> {
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string(), "reloaded".to_string());
        Ok(())
    }
}

pub type FileMetadata = (Option<u32>, Option<String>, Option<String>);

pub struct MockFiles {
    pub files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub metadata: Arc<Mutex<HashMap<String, FileMetadata>>>,
    pub directories: Arc<Mutex<HashSet<String>>>,
}

impl MockFiles {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            metadata: Arc::new(Mutex::new(HashMap::new())),
            directories: Arc::new(Mutex::new(HashSet::new())),
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
        let mut files = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut metadata = self.metadata.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

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
            owner.map(ToString::to_string),
            group.map(ToString::to_string),
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

    fn ensure_directory(
        &self,
        path: &Path,
        _mode: Option<u32>,
        _owner: Option<&str>,
        _group: Option<&str>,
    ) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        let mut directories = self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if directories.contains(&path_str) {
            Ok(false)
        } else {
            directories.insert(path_str);
            Ok(true)
        }
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        let mut files = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut metadata = self.metadata.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

        let f_removed = files.remove(&path_str).is_some();
        let m_removed = metadata.remove(&path_str).is_some();

        Ok(f_removed || m_removed)
    }
}
