use std::io::Read as _;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("CREDENTIALS_DIRECTORY environment variable not set")]
    NoDirectory,
    #[error("Invalid secret name {0:?}: must be a plain file name")]
    InvalidName(String),
    #[error("Failed to read secret {0}: {1}")]
    ReadError(String, std::io::Error),
}

pub struct CredentialManager {
    base_path: PathBuf,
}

impl CredentialManager {
    pub fn new() -> Result<Self, CredentialError> {
        let path = std::env::var("CREDENTIALS_DIRECTORY")
            .map(|s| PathBuf::from(s.trim()))
            .map_err(|_| CredentialError::NoDirectory)?;
        Ok(Self { base_path: path })
    }

    /// Read a secret by name from the credentials directory.
    ///
    /// The name must be a plain file name: path separators, parent
    /// references and absolute paths are rejected so a caller can never
    /// escape the credentials directory, even if the name ever stops
    /// being a hardcoded constant.
    pub fn read_secret(&self, name: &str) -> Result<String, CredentialError> {
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(CredentialError::InvalidName(name.to_string()));
        }

        let secret_path = self.base_path.join(name);

        let mut file = std::fs::File::open(&secret_path)
            .map_err(|e| CredentialError::ReadError(name.to_string(), e))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| CredentialError::ReadError(name.to_string(), e))?;
        // Remove only trailing whitespace, preserving other characters
        let new_len = content.trim_end().len();
        content.truncate(new_len);
        Ok(content)
    }
}

#[cfg(test)]
#[path = "credentials/tests.rs"]
mod tests;
