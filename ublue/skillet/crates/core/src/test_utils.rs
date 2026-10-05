use crate::files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource};
use crate::system::{
    AccountLookupResource, AccountResource, GroupIdentity, PodmanSecretResource, ServiceResource,
    SystemError, UserIdentity,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct MockSystem {
    pub groups: Arc<Mutex<HashSet<String>>>,
    pub users: Arc<Mutex<HashSet<String>>>,
    pub user_identities: Arc<Mutex<HashMap<String, UserIdentity>>>,
    pub group_identities: Arc<Mutex<HashMap<String, GroupIdentity>>>,
    pub podman_secrets: Arc<Mutex<HashSet<String>>>,
    pub secret_ids: Arc<Mutex<HashMap<String, String>>>,
    pub fail_restart_once: Arc<AtomicBool>,
    pub fail_start_once: Arc<AtomicBool>,
    pub fail_reload_once: Arc<AtomicBool>,
    pub restart_count: Arc<AtomicUsize>,
    pub start_count: Arc<AtomicUsize>,
    pub services: Arc<Mutex<HashMap<String, String>>>, // name -> state
}

impl MockSystem {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(Mutex::new(HashSet::new())),
            users: Arc::new(Mutex::new(HashSet::new())),
            user_identities: Arc::new(Mutex::new(HashMap::new())),
            group_identities: Arc::new(Mutex::new(HashMap::new())),
            podman_secrets: Arc::new(Mutex::new(HashSet::new())),
            secret_ids: Arc::new(Mutex::new(HashMap::new())),
            fail_restart_once: Arc::new(AtomicBool::new(false)),
            fail_start_once: Arc::new(AtomicBool::new(false)),
            fail_reload_once: Arc::new(AtomicBool::new(false)),
            restart_count: Arc::new(AtomicUsize::new(0)),
            start_count: Arc::new(AtomicUsize::new(0)),
            services: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountLookupResource for MockSystem {
    fn user_by_name(&self, name: &str) -> Result<Option<UserIdentity>, SystemError> {
        Ok(self
            .user_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned())
    }

    fn user_by_uid(&self, uid: u32) -> Result<Option<UserIdentity>, SystemError> {
        Ok(self
            .user_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .find(|user| user.uid == uid)
            .cloned())
    }

    fn group_by_name(&self, name: &str) -> Result<Option<GroupIdentity>, SystemError> {
        Ok(self
            .group_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned())
    }
}

impl AccountResource for MockSystem {
    fn ensure_group(&self, name: &str, gid: Option<u32>) -> Result<bool, SystemError> {
        let mut groups = self
            .groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut identities = self
            .group_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = identities.get(name) {
            if gid.is_some_and(|desired| desired != existing.gid) {
                return Err(SystemError::GroupCheck(format!(
                    "Group {name} exists with GID {}, requested {gid:?}",
                    existing.gid
                )));
            }
            return Ok(false);
        }
        groups.insert(name.to_string());
        identities.insert(
            name.to_string(),
            GroupIdentity {
                name: name.to_string(),
                gid: gid.unwrap_or(2000),
            },
        );
        Ok(true)
    }

    fn ensure_user(
        &self,
        name: &str,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<bool, SystemError> {
        if let Some(gid) = gid {
            self.ensure_group(name, Some(gid))?;
        }
        let mut users = self
            .users
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut identities = self
            .user_identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = identities.get(name) {
            if uid.is_some_and(|desired| desired != existing.uid)
                || gid.is_some_and(|desired| desired != existing.primary_gid)
            {
                return Err(SystemError::Command(format!(
                    "User {name} exists with UID:GID {}:{}, requested {uid:?}:{gid:?}",
                    existing.uid, existing.primary_gid
                )));
            }
            return Ok(false);
        }
        let uid = uid.unwrap_or(2000);
        let primary_gid = gid.unwrap_or(2000);
        users.insert(name.to_string());
        identities.insert(
            name.to_string(),
            UserIdentity {
                name: name.to_string(),
                uid,
                primary_gid,
            },
        );
        Ok(true)
    }
}

impl PodmanSecretResource for MockSystem {
    fn podman_secret_id(&self, name: &str) -> Result<String, SystemError> {
        self.secret_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(name)
            .cloned()
            .ok_or_else(|| SystemError::Command(format!("secret {name} missing")))
    }

    fn ensure_podman_secret(&self, name: &str, payload: &str) -> Result<bool, SystemError> {
        let mut secrets = self
            .podman_secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        secrets.insert(name.to_string());
        let id = hex::encode(Sha256::digest(payload.as_bytes()));
        let mut ids = self
            .secret_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ids.get(name) == Some(&id) {
            Ok(false)
        } else {
            ids.insert(name.to_string(), id);
            Ok(true)
        }
    }
}

impl ServiceResource for MockSystem {
    fn service_is_active(&self, name: &str) -> Result<bool, SystemError> {
        Ok(matches!(
            self.services
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(name)
                .map(String::as_str),
            Some("started" | "restarted" | "reloaded")
        ))
    }

    fn service_start(&self, name: &str) -> Result<(), SystemError> {
        self.start_count.fetch_add(1, Ordering::SeqCst);
        if self.fail_start_once.swap(false, Ordering::SeqCst) {
            return Err(SystemError::Command("injected start failure".to_string()));
        }
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
        self.restart_count.fetch_add(1, Ordering::SeqCst);
        if self.fail_restart_once.swap(false, Ordering::SeqCst) {
            return Err(SystemError::Command("injected restart failure".to_string()));
        }
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_string(), "restarted".to_string());
        Ok(())
    }

    fn service_reload(&self, name: &str) -> Result<(), SystemError> {
        let mut services = self
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match services.get(name).map(String::as_str) {
            Some("started" | "restarted" | "reloaded") => {
                services.insert(name.to_string(), "reloaded".to_string());
                Ok(())
            }
            Some(state) => Err(SystemError::Command(format!(
                "cannot reload inactive service {name} (state: {state})"
            ))),
            None => Err(SystemError::Command(format!("service {name} is missing"))),
        }
    }

    fn service_enable(&self, name: &str) -> Result<(), SystemError> {
        let mut services = self
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        services
            .entry(name.to_string())
            .or_insert_with(|| "enabled".to_string());
        Ok(())
    }

    fn daemon_reload(&self) -> Result<(), SystemError> {
        if self.fail_reload_once.swap(false, Ordering::SeqCst) {
            return Err(SystemError::Command("injected reload failure".to_string()));
        }
        self.services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert("daemon-reload".to_string(), "reloaded".to_string());
        Ok(())
    }
}

pub type FileMetadata = (Option<u32>, Ownership);
pub type DirectoryMetadata = (Option<u32>, Ownership);
pub type DirectoryMetadataMap = Arc<Mutex<HashMap<String, DirectoryMetadata>>>;
pub type BtrfsMountMap = Arc<Mutex<HashMap<String, BtrfsMount>>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BtrfsMount {
    pub device: String,
    pub root: String,
    pub filesystem: String,
}

pub struct MockFiles {
    pub files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub metadata: Arc<Mutex<HashMap<String, FileMetadata>>>,
    pub directories: Arc<Mutex<HashSet<String>>>,
    pub btrfs_mounts: BtrfsMountMap,
    pub btrfs_subvolumes: Arc<Mutex<HashSet<String>>>,
    pub directory_metadata: DirectoryMetadataMap,
    pub fail_btrfs_mount_check: Arc<AtomicBool>,
    pub fail_file_write_once: Arc<AtomicBool>,
    pub fail_directory_once: Arc<AtomicBool>,
}

impl MockFiles {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            metadata: Arc::new(Mutex::new(HashMap::new())),
            directories: Arc::new(Mutex::new(HashSet::new())),
            btrfs_mounts: Arc::new(Mutex::new(HashMap::new())),
            btrfs_subvolumes: Arc::new(Mutex::new(HashSet::new())),
            directory_metadata: Arc::new(Mutex::new(HashMap::new())),
            fail_btrfs_mount_check: Arc::new(AtomicBool::new(false)),
            fail_file_write_once: Arc::new(AtomicBool::new(false)),
            fail_directory_once: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for MockFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFiles {
    /// Declare a mount-table entry for storage contract tests.
    pub fn record_btrfs_mount(&self, path: &Path, device: &str, root: &str, filesystem: &str) {
        let path = path.display().to_string();
        self.directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(path.clone());
        self.btrfs_mounts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                path,
                BtrfsMount {
                    device: device.to_string(),
                    root: root.to_string(),
                    filesystem: filesystem.to_string(),
                },
            );
    }

    /// Declare an existing Btrfs subvolume, including its directory ancestors.
    pub fn record_btrfs_subvolume(&self, path: &Path) {
        let path = path.display().to_string();
        self.btrfs_subvolumes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(path.clone());
        let mut directories = self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut current = Path::new(&path);
        while let Some(parent) = current.parent() {
            directories.insert(parent.display().to_string());
            current = parent;
        }
        directories.insert(path);
    }
}

impl StorageResource for MockFiles {
    fn require_btrfs_subvolume_mount(
        &self,
        path: &Path,
        backing_mount: &Path,
        subvolume_root: &str,
    ) -> Result<(), FileError> {
        if self.fail_btrfs_mount_check.load(Ordering::SeqCst) {
            Err(FileError::WrongMount(path.display().to_string()))
        } else {
            let mounts = self
                .btrfs_mounts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let path_text = path.display().to_string();
            let mount = mounts
                .get(&path_text)
                .ok_or_else(|| FileError::MountMissing(path_text.clone()))?;
            let backing = mounts.get(&backing_mount.display().to_string());
            if mount.filesystem == "btrfs"
                && mount.root == subvolume_root
                && backing.is_some_and(|backing| {
                    backing.device == mount.device && backing.filesystem == "btrfs"
                })
            {
                Ok(())
            } else {
                Err(FileError::WrongMount(path_text))
            }
        }
    }

    fn require_btrfs_subvolume(&self, path: &Path) -> Result<(), FileError> {
        if self
            .btrfs_subvolumes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&path.display().to_string())
        {
            Ok(())
        } else {
            Err(FileError::NotASubvolume(path.display().to_string()))
        }
    }

    fn ensure_btrfs_subvolume(&self, path: &Path) -> Result<bool, FileError> {
        let path_text = path.display().to_string();
        if self
            .btrfs_subvolumes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&path_text)
        {
            return Ok(false);
        }
        if self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&path_text)
            || self
                .files
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_key(&path_text)
        {
            return Err(FileError::NotASubvolume(path_text));
        }
        let parent = path
            .parent()
            .ok_or_else(|| FileError::InvalidPath(path_text.clone()))?;
        if !self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&parent.display().to_string())
        {
            return Err(FileError::ParentMissing(path_text));
        }
        self.btrfs_subvolumes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(path_text.clone());
        self.directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(path_text);
        Ok(true)
    }
}

impl FileReadResource for MockFiles {
    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FileError> {
        Ok(self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&path.display().to_string())
            .cloned())
    }
}

impl FileMutationResource for MockFiles {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        if self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&path_str)
        {
            return Err(FileError::NotAFile(path_str));
        }
        if self.fail_file_write_once.swap(false, Ordering::SeqCst) {
            return Err(FileError::Io(std::io::Error::other(
                "injected file write failure",
            )));
        }
        let mut files = self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut metadata = self
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

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

        let current = metadata.get(&path_str).cloned().unwrap_or_default();
        let desired = (
            mode.or(current.0),
            Ownership {
                uid: ownership.uid.clone().or(current.1.uid),
                gid: ownership.gid.clone().or(current.1.gid),
            },
        );
        let unchanged = !changed && metadata.get(&path_str) == Some(&desired);
        metadata.insert(path_str, desired);
        changed |= !unchanged;

        Ok(changed)
    }

    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        if self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&path_str)
        {
            return Err(FileError::NotADirectory(path_str));
        }
        if self.fail_directory_once.swap(false, Ordering::SeqCst) {
            return Err(FileError::Io(std::io::Error::other(
                "injected directory failure",
            )));
        }
        let mut directories = self
            .directories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let was_present = directories.contains(&path_str);
        directories.insert(path_str.clone());
        drop(directories);
        let mut metadata = self
            .directory_metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = metadata.get(&path_str).cloned().unwrap_or_default();
        let desired = (
            mode.or(current.0),
            Ownership {
                uid: ownership.uid.clone().or(current.1.uid),
                gid: ownership.gid.clone().or(current.1.gid),
            },
        );
        let unchanged = was_present && metadata.get(&path_str) == Some(&desired);
        metadata.insert(path_str, desired);
        Ok(!unchanged)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        let path_str = path.display().to_string();
        let mut files = self
            .files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut metadata = self
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let f_removed = files.remove(&path_str).is_some();
        let m_removed = metadata.remove(&path_str).is_some();

        Ok(f_removed || m_removed)
    }
}
