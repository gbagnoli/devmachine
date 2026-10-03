//! Workstation VM identity and durable ownership. No vault or guest resources.

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
}

pub type Result<T> = std::result::Result<T, Error>;
