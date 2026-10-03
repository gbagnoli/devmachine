//! Workstation VM identity and durable ownership. No vault or guest resources.

pub mod artifacts;
pub mod backend;
pub mod catalog;
pub mod creation;
pub mod delivery;
pub mod domain_xml;
pub mod lifecycle;
pub mod manifest;
mod process;
pub mod provisioning;
pub mod readiness;
pub mod staging;
pub mod transport;

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
    #[error("libvirt XML rendering failed: {0}")]
    XmlWrite(#[from] quick_xml::Error),
    #[error("libvirt command {operation} failed (exit {code:?}); inspect the selected runtime")]
    Command {
        operation: String,
        code: Option<i32>,
    },
    #[error("VM tooling command exceeded its timeout")]
    Timeout,
    #[error("VM artifact preparation failed: {0}")]
    Preparation(String),
    #[error("external VM cleanup failed: {0}")]
    ExternalCleanup(String),
    #[error("VM run is busy; retry after its current operation finishes")]
    Busy,
    #[error("Cargo build failed (exit {code:?}): {diagnostic}")]
    Build {
        code: Option<i32>,
        diagnostic: String,
    },
    #[error(
        "guest operation {operation} failed (exit {code:?}); inspect retained guest diagnostics"
    )]
    Guest {
        operation: String,
        code: Option<i32>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub use process::capture_version;

pub fn current_uid() -> u32 {
    users::get_current_uid()
}
