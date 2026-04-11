use std::io::Read as _;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CredentialError {
    #[error("CREDENTIALS_DIRECTORY environment variable not set")]
    NoDirectory,
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

    pub fn read_secret(&self, name: &str) -> Result<String, CredentialError> {
        let secret_path = self.base_path.join(name);

        let mut file = std::fs::File::open(&secret_path)
            .map_err(|e| CredentialError::ReadError(name.to_string(), e))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| CredentialError::ReadError(name.to_string(), e))?;
        Ok(content.trim().to_string())
    }
}
