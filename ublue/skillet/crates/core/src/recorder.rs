use crate::files::{FileError, FileResource};
use crate::resource_op::ResourceOp;
use crate::system::{SystemError, SystemResource};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct Recorder<T> {
    inner: T,
    ops: Arc<Mutex<Vec<ResourceOp>>>,
}

impl<T> Recorder<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            ops: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_ops(inner: T, ops: Arc<Mutex<Vec<ResourceOp>>>) -> Self {
        Self { inner, ops }
    }

    pub fn get_ops(&self) -> Vec<ResourceOp> {
        self.ops.lock().unwrap().clone()
    }

    pub fn shared_ops(&self) -> Arc<Mutex<Vec<ResourceOp>>> {
        self.ops.clone()
    }

    fn record(&self, op: ResourceOp) {
        self.ops.lock().unwrap().push(op);
    }
}

impl<T: FileResource> FileResource for Recorder<T> {
    fn ensure_file(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
    ) -> Result<bool, FileError> {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = hex::encode(hasher.finalize());

        self.record(ResourceOp::EnsureFile {
            path: path.display().to_string(),
            content_hash: hash,
            mode,
            owner: owner.map(|s| s.to_string()),
            group: group.map(|s| s.to_string()),
        });

        self.inner.ensure_file(path, content, mode, owner, group)
    }

    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        self.record(ResourceOp::DeleteFile {
            path: path.display().to_string(),
        });
        self.inner.delete_file(path)
    }
}

impl<T: SystemResource> SystemResource for Recorder<T> {
    fn ensure_group(&self, name: &str) -> Result<bool, SystemError> {
        self.record(ResourceOp::EnsureGroup {
            name: name.to_string(),
        });
        self.inner.ensure_group(name)
    }
}
