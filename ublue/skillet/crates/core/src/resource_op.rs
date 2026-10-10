use crate::files::Ownership;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ResourceOp {
    EnsureFile {
        path: String,
        mode: Option<String>,
        #[serde(default)]
        ownership: Ownership,
    },
    DeleteFile {
        path: String,
    },
    EnsureDirectory {
        path: String,
        mode: Option<String>,
        #[serde(default)]
        ownership: Ownership,
    },
    EnsureBtrfsSubvolume {
        path: String,
    },
    EnsureGroup {
        name: String,
        gid: Option<u32>,
    },
    EnsureUser {
        name: String,
        uid: Option<u32>,
        gid: Option<u32>,
    },
    EnsurePodmanSecret {
        name: String,
    },
    ServiceStart {
        name: String,
    },
    ServiceStop {
        name: String,
    },
    ServiceRestart {
        name: String,
    },
    ServiceReload {
        name: String,
    },
    ServiceEnable {
        name: String,
    },
    DaemonReload,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(tag = "status", content = "changed", rename_all = "snake_case")]
pub enum EffectResult {
    Changed(bool),
    Succeeded,
    Failed,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct RecordedOperation {
    pub operation: ResourceOp,
    pub result: EffectResult,
}
