use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ResourceOp {
    EnsureFile {
        path: String,
        content_hash: String,
        mode: Option<String>,
        owner: Option<String>,
        group: Option<String>,
    },
    DeleteFile {
        path: String,
    },
    EnsureDirectory {
        path: String,
        mode: Option<String>,
        owner: Option<String>,
        group: Option<String>,
    },
    EnsureGroup {
        name: String,
    },
    EnsureUser {
        name: String,
        uid: Option<u32>,
        gid: Option<u32>,
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
}
