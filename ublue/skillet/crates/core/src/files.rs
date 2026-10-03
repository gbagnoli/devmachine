use nix::unistd::{chown, fchown, Gid, Uid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;
use thiserror::Error;
use tracing::info;
use users::{get_group_by_name, get_user_by_name};

#[derive(Error, Debug)]
pub enum FileError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to persist temporary file to {0}: {1}")]
    Persist(String, io::Error),
    #[error("Failed to read existing file {0}: {1}")]
    Read(String, io::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Parent directory for {0} does not exist")]
    ParentMissing(String),
    #[error("Failed to set permissions for {0}: {1}")]
    SetPermissions(String, io::Error),
    #[error("Failed to set ownership for {0}: {1}")]
    SetOwnership(String, String),
    #[error("User {0} not found")]
    UserNotFound(String),
    #[error("Group {0} not found")]
    GroupNotFound(String),
    #[error("Path {0} exists but is not a directory")]
    NotADirectory(String),
    #[error("Path {0} exists but is not a regular file")]
    NotAFile(String),
    #[error("Metadata mismatch for {0}")]
    Metadata(String),
    #[error("Required data mount is absent: {0}")]
    MountMissing(String),
    #[error("Unexpected filesystem or subvolume at {0}")]
    WrongMount(String),
    #[error("Path {0} exists but is not a Btrfs subvolume")]
    NotASubvolume(String),
    #[error("Btrfs operation failed for {0}: {1}")]
    Btrfs(String, String),
}

/// Owner identity can be expressed by a host account name or a numeric ID.
/// Numeric identities do not require a matching NSS entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OwnerIdentity {
    Name(String),
    Id(u32),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ownership {
    pub uid: Option<OwnerIdentity>,
    pub gid: Option<OwnerIdentity>,
}

pub trait FileReadResource {
    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FileError>;
}

pub trait FileMutationResource {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError>;
    fn ensure_directory_with_ownership(
        &self,
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError>;
    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        self.ensure_directory_with_ownership(
            path,
            mode,
            &Ownership {
                uid: owner.map(|name| OwnerIdentity::Name(name.to_string())),
                gid: group.map(|name| OwnerIdentity::Name(name.to_string())),
            },
        )
    }
    fn delete_file(&self, path: &Path) -> Result<bool, FileError>;
}

pub trait StorageResource {
    fn require_btrfs_subvolume_mount(
        &self,
        path: &Path,
        backing_mount: &Path,
        subvolume_root: &str,
    ) -> Result<(), FileError>;
    fn require_btrfs_subvolume(&self, path: &Path) -> Result<(), FileError>;
    fn ensure_btrfs_subvolume(&self, path: &Path) -> Result<bool, FileError>;
}

/// Transitional aggregate for recipes that still consume more than one file
/// capability. New consumers should request the narrowest trait they need.
pub trait FileResource: FileReadResource + FileMutationResource + StorageResource {}

impl<T> FileResource for T where T: FileReadResource + FileMutationResource + StorageResource {}

pub struct LocalFileResource;

impl LocalFileResource {
    pub fn new() -> Self {
        Self
    }

    fn check_metadata(
        path: &Path,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        let metadata =
            fs::metadata(path).map_err(|e| FileError::Read(path.display().to_string(), e))?;
        let mut changed = false;

        if let Some(desired_mode) = mode {
            if (metadata.permissions().mode() & 0o7777) != desired_mode {
                changed = true;
            }
        }

        if let Some(desired_user) = owner {
            let user = get_user_by_name(desired_user)
                .ok_or_else(|| FileError::UserNotFound(desired_user.to_string()))?;
            if metadata.uid() != user.uid() {
                changed = true;
            }
        }

        if let Some(desired_group) = group {
            let grp = get_group_by_name(desired_group)
                .ok_or_else(|| FileError::GroupNotFound(desired_group.to_string()))?;
            if metadata.gid() != grp.gid() {
                changed = true;
            }
        }

        Ok(changed)
    }

    fn apply_metadata_to_file(
        file: &File,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<(), FileError> {
        use std::os::unix::io::AsRawFd;

        if let Some(m) = mode {
            let mut perms = file.metadata().map_err(FileError::Io)?.permissions();
            perms.set_mode(m);
            file.set_permissions(perms).map_err(FileError::Io)?;
        }

        if owner.is_some() || group.is_some() {
            let uid = owner
                .map(|u| get_user_by_name(u).ok_or_else(|| FileError::UserNotFound(u.to_string())))
                .transpose()?
                .map(|u| Uid::from_raw(u.uid()));

            let gid = group
                .map(|g| {
                    get_group_by_name(g).ok_or_else(|| FileError::GroupNotFound(g.to_string()))
                })
                .transpose()?
                .map(|g| Gid::from_raw(g.gid()));

            fchown(file.as_raw_fd(), uid, gid)
                .map_err(|e| FileError::SetOwnership("temp file".to_string(), e.to_string()))?;
        }

        Ok(())
    }

    fn apply_metadata(
        path: &Path,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<(), FileError> {
        if let Some(desired_mode) = mode {
            let mut perms = fs::metadata(path)
                .map_err(|e| FileError::Read(path.display().to_string(), e))?
                .permissions();
            perms.set_mode(desired_mode);
            fs::set_permissions(path, perms)
                .map_err(|e| FileError::SetPermissions(path.display().to_string(), e))?;
        }

        if owner.is_some() || group.is_some() {
            let uid = owner
                .map(|u| get_user_by_name(u).ok_or_else(|| FileError::UserNotFound(u.to_string())))
                .transpose()?
                .map(|u| Uid::from_raw(u.uid()));

            let gid = group
                .map(|g| {
                    get_group_by_name(g).ok_or_else(|| FileError::GroupNotFound(g.to_string()))
                })
                .transpose()?
                .map(|g| Gid::from_raw(g.gid()));

            chown(path, uid, gid)
                .map_err(|e| FileError::SetOwnership(path.display().to_string(), e.to_string()))?;
        }

        Ok(())
    }

    fn identity_uid(identity: &OwnerIdentity) -> Result<u32, FileError> {
        match identity {
            OwnerIdentity::Name(name) => get_user_by_name(name)
                .map(|user| user.uid())
                .ok_or_else(|| FileError::UserNotFound(name.clone())),
            OwnerIdentity::Id(uid) => Ok(*uid),
        }
    }

    fn identity_gid(identity: &OwnerIdentity) -> Result<u32, FileError> {
        match identity {
            OwnerIdentity::Name(name) => get_group_by_name(name)
                .map(|group| group.gid())
                .ok_or_else(|| FileError::GroupNotFound(name.clone())),
            OwnerIdentity::Id(gid) => Ok(*gid),
        }
    }

    fn check_directory_metadata(
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let metadata = fs::metadata(path)
            .map_err(|error| FileError::Read(path.display().to_string(), error))?;
        let uid = ownership.uid.as_ref().map(Self::identity_uid).transpose()?;
        let gid = ownership.gid.as_ref().map(Self::identity_gid).transpose()?;
        Ok(
            mode.is_some_and(|desired| metadata.permissions().mode() & 0o7777 != desired)
                || uid.is_some_and(|desired| metadata.uid() != desired)
                || gid.is_some_and(|desired| metadata.gid() != desired),
        )
    }

    fn apply_directory_metadata(
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<(), FileError> {
        if let Some(mode) = mode {
            let mut permissions = fs::metadata(path)
                .map_err(|error| FileError::Read(path.display().to_string(), error))?
                .permissions();
            permissions.set_mode(mode);
            fs::set_permissions(path, permissions)
                .map_err(|error| FileError::SetPermissions(path.display().to_string(), error))?;
        }
        if ownership.uid.is_some() || ownership.gid.is_some() {
            let uid = ownership
                .uid
                .as_ref()
                .map(Self::identity_uid)
                .transpose()?
                .map(Uid::from_raw);
            let gid = ownership
                .gid
                .as_ref()
                .map(Self::identity_gid)
                .transpose()?
                .map(Gid::from_raw);
            chown(path, uid, gid).map_err(|error| {
                FileError::SetOwnership(path.display().to_string(), error.to_string())
            })?;
        }
        Ok(())
    }

    fn get_file_hash(path: &Path) -> Result<Vec<u8>, FileError> {
        let file = File::open(path).map_err(|e| FileError::Read(path.display().to_string(), e))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        io::copy(&mut reader, &mut hasher)
            .map_err(|e| FileError::Read(path.display().to_string(), e))?;
        Ok(hasher.finalize().to_vec())
    }
}

impl Default for LocalFileResource {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageResource for LocalFileResource {
    fn require_btrfs_subvolume_mount(
        &self,
        path: &Path,
        backing_mount: &Path,
        subvolume_root: &str,
    ) -> Result<(), FileError> {
        let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
        require_btrfs_mount_in(&mountinfo, path, backing_mount, subvolume_root)
    }

    fn require_btrfs_subvolume(&self, path: &Path) -> Result<(), FileError> {
        let output = Command::new("btrfs")
            .args(["subvolume", "show"])
            .arg(path)
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(FileError::NotASubvolume(path.display().to_string()))
        }
    }

    fn ensure_btrfs_subvolume(&self, path: &Path) -> Result<bool, FileError> {
        let path_text = path.display().to_string();
        if Command::new("btrfs")
            .args(["subvolume", "show"])
            .arg(path)
            .output()?
            .status
            .success()
        {
            return Ok(false);
        }
        if path.exists() {
            return Err(FileError::NotASubvolume(path_text));
        }
        let parent = path
            .parent()
            .ok_or_else(|| FileError::InvalidPath(path_text.clone()))?;
        if !parent.is_dir() {
            return Err(FileError::ParentMissing(path_text));
        }
        let output = Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err(FileError::Btrfs(
                path_text,
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
        Ok(true)
    }
}

impl FileReadResource for LocalFileResource {
    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FileError> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(FileError::Read(path.display().to_string(), error)),
        }
    }
}

impl FileMutationResource for LocalFileResource {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        // 1. Check parent directory
        let parent = path
            .parent()
            .ok_or_else(|| FileError::InvalidPath(path.display().to_string()))?;

        if !parent.exists() {
            return Err(FileError::ParentMissing(path.display().to_string()));
        }

        let mut changed = false;

        // 2. Check content
        let content_changed = if path.exists() {
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| FileError::Read(path.display().to_string(), e))?;

            // If it's a symlink, we replace it with a regular file
            if metadata.is_file() {
                if metadata.len() == content.len() as u64 {
                    let existing_hash = Self::get_file_hash(path)?;
                    let mut hasher = Sha256::new();
                    hasher.update(content);
                    let new_hash = hasher.finalize();
                    existing_hash != new_hash.as_slice()
                } else {
                    true
                }
            } else {
                // Not a regular file (symlink or dir), we will delete and replace
                self.delete_file(path)?;
                true
            }
        } else {
            true
        };

        if content_changed {
            // Write to temp file in same directory (for atomic rename)
            let mut temp_file = NamedTempFile::new_in(parent)?;
            temp_file.write_all(content)?;
            // Apply metadata to temp file before persist
            Self::apply_metadata_to_file(temp_file.as_file(), mode, owner, group)?;
            temp_file
                .persist(path)
                .map_err(|e| FileError::Persist(path.display().to_string(), e.error))?;
            changed = true;
            info!("Updated file content for {}", path.display());
        } else {
            // Even if content didn't change, we might need to update metadata
            if path.exists() && Self::check_metadata(path, mode, owner, group)? {
                Self::apply_metadata(path, mode, owner, group)?;
                changed = true;
                info!("Updated file metadata for {}", path.display());
            }
        }

        Ok(changed)
    }

    fn ensure_directory_with_ownership(
        &self,
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let mut changed = false;

        let metadata_res = fs::symlink_metadata(path);
        match metadata_res {
            Ok(metadata) => {
                // Path exists, check if it's a directory
                if !metadata.is_dir() {
                    // If it's a symlink, follow it to see if it points to a directory
                    let followed_metadata_res = fs::metadata(path);
                    match followed_metadata_res {
                        Ok(fm) if fm.is_dir() => {
                            // Points to a directory, fine
                        }
                        _ => {
                            // Doesn't exist or not a directory
                            return Err(FileError::NotADirectory(path.display().to_string()));
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Path does not exist, create it
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true);
                if let Some(m) = mode {
                    builder.mode(m);
                }
                builder.create(path).map_err(FileError::Io)?;
                changed = true;
                info!("Created directory {}", path.display());
            }
            Err(e) => {
                return Err(FileError::Read(path.display().to_string(), e));
            }
        }

        if path.exists() && Self::check_directory_metadata(path, mode, ownership)? {
            Self::apply_directory_metadata(path, mode, ownership)?;
            changed = true;
            info!("Updated directory metadata for {}", path.display());
        }

        Ok(changed)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        if path.exists() {
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| FileError::Read(path.display().to_string(), e))?;
            if metadata.is_dir() {
                fs::remove_dir_all(path).map_err(FileError::Io)?;
            } else {
                fs::remove_file(path).map_err(FileError::Io)?;
            }
            info!("Deleted {}", path.display());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn mount_entry<'a>(mountinfo: &'a str, path: &str) -> Option<(&'a str, &'a str, &'a str)> {
    mountinfo.lines().find_map(|line| {
        let (left, right) = line.split_once(" - ")?;
        let mut fields = left.split_whitespace();
        let device = fields.nth(2)?;
        let root = fields.next()?;
        let mountpoint = fields.next()?;
        let filesystem = right.split_whitespace().next()?;
        (mountpoint == path).then_some((device, root, filesystem))
    })
}

fn require_btrfs_mount_in(
    mountinfo: &str,
    path: &Path,
    backing_mount: &Path,
    subvolume_root: &str,
) -> Result<(), FileError> {
    let path_text = path
        .to_str()
        .ok_or_else(|| FileError::InvalidPath(path.display().to_string()))?;
    let backing_text = backing_mount
        .to_str()
        .ok_or_else(|| FileError::InvalidPath(backing_mount.display().to_string()))?;
    let (device, root, filesystem) = mount_entry(mountinfo, path_text)
        .ok_or_else(|| FileError::MountMissing(path_text.to_string()))?;
    let backing = mount_entry(mountinfo, backing_text);
    if filesystem == "btrfs"
        && root == subvolume_root
        && backing.is_some_and(|(backing_device, _, backing_fs)| {
            backing_device == device && backing_fs == "btrfs"
        })
    {
        Ok(())
    } else {
        Err(FileError::WrongMount(path_text.to_string()))
    }
}

#[cfg(test)]
#[path = "files/tests.rs"]
mod tests;
