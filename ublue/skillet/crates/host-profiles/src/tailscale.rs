//! Typed encrypted enrollment payload; clear policy is resolved before delivery.
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("invalid Tailscale enrollment payload")]
pub struct InputError;

// No Debug: auth_key is sensitive.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentInput {
    version: u8,
    pub auth_key: String,
    pub advertise_exit_node: bool,
}

impl EnrollmentInput {
    pub fn new(auth_key: String, advertise_exit_node: bool) -> Self {
        Self {
            version: 1,
            auth_key,
            advertise_exit_node,
        }
    }
    pub fn parse(payload: &str) -> Result<Self, InputError> {
        let input = if payload.trim_start().starts_with('{') {
            serde_json::from_str(payload).map_err(|_| InputError)?
        } else {
            // Retained raw credentials stay usable without implicitly enabling routing.
            Self::new(payload.trim().to_string(), false)
        };
        input.validate()?;
        Ok(input)
    }
    pub fn payload(&self) -> Result<String, InputError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| InputError)
    }
    fn validate(&self) -> Result<(), InputError> {
        if self.version != 1
            || self.auth_key.trim().is_empty()
            || self.auth_key.contains(['\n', '\r', '\0'])
        {
            return Err(InputError);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tailscale/tests.rs"]
mod tests;
