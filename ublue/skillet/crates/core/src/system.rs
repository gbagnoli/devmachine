use std::process::Command;
use thiserror::Error;
use tracing::{debug, info};
use users::get_group_by_name;

#[derive(Error, Debug)]
pub enum SystemError {
    #[error("Group check error: {0}")]
    GroupCheck(String),
    #[error("Command failed: {0}")]
    Command(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub trait SystemResource {
    fn ensure_group(&self, name: &str) -> Result<bool, SystemError>;
}

pub struct LinuxSystemResource;

impl LinuxSystemResource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxSystemResource {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemResource for LinuxSystemResource {
    fn ensure_group(&self, name: &str) -> Result<bool, SystemError> {
        // 1. Check if group exists using `users` crate
        if get_group_by_name(name).is_some() {
            debug!("Group {} already exists", name);
            return Ok(false);
        }

        // 2. Create group using `groupadd`
        // Note: Creating groups requires root privileges usually.
        info!("Creating group {}", name);
        let output = Command::new("groupadd")
            .arg(name)
            // .arg("-r") // System group? Maybe make it an option?
            // For now, simple group creation.
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SystemError::Command(format!("groupadd failed: {}", stderr)));
        }

        info!("Created group {}", name);
        Ok(true)
    }
}

#[cfg(test)]
#[path = "system/tests.rs"]
mod tests;
