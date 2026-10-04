use crate::files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource};
use crate::resource_op::{EffectResult, RecordedOperation, ResourceOp};
use crate::system::{
    AccountLookupResource, AccountResource, GroupIdentity, PodmanSecretResource, ServiceResource,
    SystemError, UserIdentity,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct Recorder<T> {
    inner: T,
    ops: Arc<Mutex<Vec<RecordedOperation>>>,
}

impl<T> Recorder<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            ops: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_ops(inner: T, ops: Arc<Mutex<Vec<RecordedOperation>>>) -> Self {
        Self { inner, ops }
    }

    pub fn get_ops(&self) -> Vec<RecordedOperation> {
        self.ops
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn shared_ops(&self) -> Arc<Mutex<Vec<RecordedOperation>>> {
        self.ops.clone()
    }

    fn record(&self, operation: ResourceOp, result: EffectResult) {
        self.ops
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(RecordedOperation { operation, result });
    }
}

fn changed<E>(result: &Result<bool, E>) -> EffectResult {
    result
        .as_ref()
        .map_or(EffectResult::Failed, |value| EffectResult::Changed(*value))
}

fn succeeded<T, E>(result: &Result<T, E>) -> EffectResult {
    if result.is_ok() {
        EffectResult::Succeeded
    } else {
        EffectResult::Failed
    }
}

impl<T: StorageResource> StorageResource for Recorder<T> {
    fn require_btrfs_subvolume_mount(
        &self,
        path: &Path,
        backing_mount: &Path,
        subvolume_root: &str,
    ) -> Result<(), FileError> {
        self.inner
            .require_btrfs_subvolume_mount(path, backing_mount, subvolume_root)
    }
    fn require_btrfs_subvolume(&self, path: &Path) -> Result<(), FileError> {
        self.inner.require_btrfs_subvolume(path)
    }
    fn ensure_btrfs_subvolume(&self, path: &Path) -> Result<bool, FileError> {
        let result = self.inner.ensure_btrfs_subvolume(path);
        self.record(
            ResourceOp::EnsureBtrfsSubvolume {
                path: path.display().to_string(),
            },
            changed(&result),
        );
        result
    }
}

impl<T: FileReadResource> FileReadResource for Recorder<T> {
    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FileError> {
        self.inner.read_file(path)
    }
}

impl<T: FileMutationResource> FileMutationResource for Recorder<T> {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let result = self.inner.ensure_file(path, content, mode, ownership);
        self.record(
            ResourceOp::EnsureFile {
                path: path.display().to_string(),
                mode: mode.map(|m| format!("0o{m:o}")),
                ownership: ownership.clone(),
            },
            changed(&result),
        );
        result
    }
    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        ownership: &Ownership,
    ) -> Result<bool, FileError> {
        let result = self.inner.ensure_directory(path, mode, ownership);
        self.record(
            ResourceOp::EnsureDirectory {
                path: path.display().to_string(),
                mode: mode.map(|m| format!("0o{m:o}")),
                ownership: ownership.clone(),
            },
            changed(&result),
        );
        result
    }
    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        let result = self.inner.delete_file(path);
        self.record(
            ResourceOp::DeleteFile {
                path: path.display().to_string(),
            },
            changed(&result),
        );
        result
    }
}

impl<T: PodmanSecretResource> PodmanSecretResource for Recorder<T> {
    fn podman_secret_id(&self, name: &str) -> Result<String, SystemError> {
        self.inner.podman_secret_id(name)
    }
    fn ensure_podman_secret(&self, name: &str, payload: &str) -> Result<bool, SystemError> {
        let result = self.inner.ensure_podman_secret(name, payload);
        self.record(
            ResourceOp::EnsurePodmanSecret {
                name: name.to_string(),
            },
            changed(&result),
        );
        result
    }
}

impl<T: ServiceResource> ServiceResource for Recorder<T> {
    fn service_is_active(&self, name: &str) -> Result<bool, SystemError> {
        self.inner.service_is_active(name)
    }
    fn service_start(&self, name: &str) -> Result<(), SystemError> {
        let result = self.inner.service_start(name);
        self.record(
            ResourceOp::ServiceStart {
                name: name.to_string(),
            },
            succeeded(&result),
        );
        result
    }
    fn service_stop(&self, name: &str) -> Result<(), SystemError> {
        let result = self.inner.service_stop(name);
        self.record(
            ResourceOp::ServiceStop {
                name: name.to_string(),
            },
            succeeded(&result),
        );
        result
    }
    fn service_restart(&self, name: &str) -> Result<(), SystemError> {
        let result = self.inner.service_restart(name);
        self.record(
            ResourceOp::ServiceRestart {
                name: name.to_string(),
            },
            succeeded(&result),
        );
        result
    }
    fn service_reload(&self, name: &str) -> Result<(), SystemError> {
        let result = self.inner.service_reload(name);
        self.record(
            ResourceOp::ServiceReload {
                name: name.to_string(),
            },
            succeeded(&result),
        );
        result
    }
    fn service_enable(&self, name: &str) -> Result<(), SystemError> {
        let result = self.inner.service_enable(name);
        self.record(
            ResourceOp::ServiceEnable {
                name: name.to_string(),
            },
            succeeded(&result),
        );
        result
    }
    fn daemon_reload(&self) -> Result<(), SystemError> {
        let result = self.inner.daemon_reload();
        self.record(ResourceOp::DaemonReload, succeeded(&result));
        result
    }
}

impl<T: AccountLookupResource> AccountLookupResource for Recorder<T> {
    fn user_by_name(&self, name: &str) -> Result<Option<UserIdentity>, SystemError> {
        self.inner.user_by_name(name)
    }
    fn user_by_uid(&self, uid: u32) -> Result<Option<UserIdentity>, SystemError> {
        self.inner.user_by_uid(uid)
    }
    fn group_by_name(&self, name: &str) -> Result<Option<GroupIdentity>, SystemError> {
        self.inner.group_by_name(name)
    }
}

impl<T: AccountResource> AccountResource for Recorder<T> {
    fn ensure_group(&self, name: &str, gid: Option<u32>) -> Result<bool, SystemError> {
        let result = self.inner.ensure_group(name, gid);
        self.record(
            ResourceOp::EnsureGroup {
                name: name.to_string(),
                gid,
            },
            changed(&result),
        );
        result
    }
    fn ensure_user(
        &self,
        name: &str,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<bool, SystemError> {
        let result = self.inner.ensure_user(name, uid, gid);
        self.record(
            ResourceOp::EnsureUser {
                name: name.to_string(),
                uid,
                gid,
            },
            changed(&result),
        );
        result
    }
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
