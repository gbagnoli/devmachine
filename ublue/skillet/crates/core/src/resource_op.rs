use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
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
    EnsureGroup {
        name: String,
    },
}
