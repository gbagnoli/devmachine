//! Cleanup boundary for a supervised activation attempt.

use thiserror::Error;

/// Implementations must hold staging and configuration exclusion for their
/// entire lifetime. Process-death recovery belongs to the supervising service.
pub trait Activation {
    type Error: std::fmt::Display;
    fn is_armed(&mut self) -> Result<bool, Self::Error>;
    fn prepare(&mut self) -> Result<(), Self::Error>;
    fn load(&mut self) -> Result<(), Self::Error>;
    fn handoff(&mut self) -> Result<(), Self::Error>;
    fn unload(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttemptError {
    #[error("a kernel was already armed before this attempt")]
    ForeignKernel,
    #[error("activation failed: {0}")]
    Operation(String),
    #[error("activation ended: {attempt}; kernel cleanup failed: {cleanup}")]
    Cleanup { attempt: String, cleanup: String },
}

/// Run under an independent process supervisor. A returning handoff is an
/// abandoned attempt: acceptance by systemd alone does not establish a reboot.
/// This boundary handles ordinary errors; it cannot recover from process death.
pub fn attempt(activation: &mut impl Activation) -> Result<(), AttemptError> {
    if activation.is_armed().map_err(operation)? {
        return Err(AttemptError::ForeignKernel);
    }
    activation.prepare().map_err(operation)?;
    // A failed loader may still have armed the kernel. Cleanup is required even
    // when load reports failure, and must not be hidden by the original error.
    let result = activation.load().and_then(|()| activation.handoff());
    let detail = match result {
        Ok(()) => "handoff returned without completing the reboot".to_owned(),
        Err(error) => error.to_string(),
    };
    let cleanup = activation.unload().and_then(|()| activation.is_armed());
    match cleanup {
        Ok(false) => Err(AttemptError::Operation(detail)),
        Ok(true) => Err(AttemptError::Cleanup {
            attempt: detail,
            cleanup: "kernel remains armed".into(),
        }),
        Err(error) => Err(AttemptError::Cleanup {
            attempt: detail,
            cleanup: error.to_string(),
        }),
    }
}

fn operation(error: impl std::fmt::Display) -> AttemptError {
    AttemptError::Operation(error.to_string())
}
