//! Workstation VM identity and durable ownership. No vault or guest resources.

pub mod backend;
pub mod lifecycle;
pub mod manifest;

pub use manifest::{
    Backend, Connection, Environment, ManifestStore, Phase, RunIdentity, SshTarget, VmRun,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid VM state: {0}")]
    Invalid(String),
    #[error("VM artifact I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("VM manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid libvirt XML: {0}")]
    Xml(#[from] quick_xml::DeError),
    #[error("libvirt command {operation} failed (exit {code:?}); inspect the selected runtime")]
    Command {
        operation: String,
        code: Option<i32>,
    },
    #[error("libvirt command exceeded its timeout")]
    Timeout,
    #[error("external VM cleanup failed: {0}")]
    ExternalCleanup(String),
    #[error("VM run is busy; retry after its current operation finishes")]
    Busy,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn current_uid() -> u32 {
    users::get_current_uid()
}
