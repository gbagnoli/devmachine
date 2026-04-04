use nix::unistd::{chown, Gid, Uid};
use sha2::{Digest, Sha256};
use std::fs::{self};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
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
    NotARegularFile(String),
}

pub trait FileResource {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError>;
    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError>;
    fn delete_file(&self, path: &Path) -> Result<bool, FileError>;
}

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
}

impl Default for LocalFileResource {
    fn default() -> Self {
        Self::new()
    }
}

impl FileResource for LocalFileResource {
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

            if !metadata.is_file() {
                return Err(FileError::NotARegularFile(path.display().to_string()));
            }

            if metadata.len() == content.len() as u64 {
                let file = fs::File::open(path)
                    .map_err(|e| FileError::Read(path.display().to_string(), e))?;
                let mut reader = std::io::BufReader::new(file);
                let mut hasher = Sha256::new();
                
                let mut buffer = [0; 8192];
                while let Ok(n) = reader.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let existing_hash = hasher.finalize();

                let mut new_hasher = Sha256::new();
                new_hasher.update(content);
                let new_hash = new_hasher.finalize();

                existing_hash != new_hash
            } else {
                true
            }
        } else {
            true
        };

        if content_changed {
            // Write to temp file in same directory (for atomic rename)
            let mut temp_file = NamedTempFile::new_in(parent)?;
            temp_file.write_all(content)?;
            temp_file
                .persist(path)
                .map_err(|e| FileError::Persist(path.display().to_string(), e.error))?;
            changed = true;
            info!("Updated file content for {}", path.display());
        }

        // 3. Check and apply metadata
        if path.exists() && Self::check_metadata(path, mode, owner, group)? {
            Self::apply_metadata(path, mode, owner, group)?;
            changed = true;
            info!("Updated file metadata for {}", path.display());
        }

        Ok(changed)
    }

    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        let mut changed = false;

        if path.exists() {
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| FileError::Read(path.display().to_string(), e))?;
            if !metadata.is_dir() {
                return Err(FileError::NotADirectory(path.display().to_string()));
            }
        } else {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            if let Some(m) = mode {
                builder.mode(m);
            }
            builder.create(path).map_err(FileError::Io)?;
            changed = true;
            info!("Created directory {}", path.display());
        }

        if path.exists() && Self::check_metadata(path, mode, owner, group)? {
            Self::apply_metadata(path, mode, owner, group)?;
            changed = true;
            info!("Updated directory metadata for {}", path.display());
        }

        Ok(changed)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        if path.exists() {
            fs::remove_file(path).map_err(FileError::Io)?;
            info!("Deleted file {}", path.display());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
#[path = "files/tests.rs"]
mod tests;
