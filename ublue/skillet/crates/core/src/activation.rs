use crate::{
    files::{FileError, FileMutationResource, FileReadResource},
    system::{ServiceResource, SystemError},
};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumerKind {
    Persistent,
    OneShot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationAction {
    None,
    Started,
    Restarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationOutcome {
    pub definition_changed: bool,
    pub action: ActivationAction,
    pub recovered_pending_activation: bool,
}

pub struct ActivationRequest {
    pub service: String,
    pub state_path: PathBuf,
    pub revision: Vec<u8>,
    pub definition_changed: bool,
    pub reload_daemon: bool,
    pub consumer_kind: ConsumerKind,
}

#[derive(Debug, Error)]
pub enum ActivationError {
    #[error("file operation failed: {0}")]
    File(#[from] FileError),
    #[error("system operation failed: {0}")]
    System(#[from] SystemError),
}

pub fn activate<S, F>(
    system: &S,
    files: &F,
    request: &ActivationRequest,
) -> Result<ActivationOutcome, ActivationError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let applied = files.read_file(&request.state_path)?;
    let pending = applied.as_deref() != Some(request.revision.as_slice());
    let needs_activation = request.definition_changed || pending;

    if needs_activation && request.reload_daemon {
        system.daemon_reload()?;
    }

    let active = system.service_is_active(&request.service)?;
    let action = match (request.consumer_kind, needs_activation, active) {
        (ConsumerKind::Persistent, true, _) | (ConsumerKind::OneShot, true, true) => {
            system.service_restart(&request.service)?;
            ActivationAction::Restarted
        }
        (ConsumerKind::Persistent, false, false) | (ConsumerKind::OneShot, true, false) => {
            system.service_start(&request.service)?;
            ActivationAction::Started
        }
        _ => ActivationAction::None,
    };

    if needs_activation {
        files.ensure_file(
            &request.state_path,
            &request.revision,
            Some(0o644),
            &crate::files::Ownership::named(Some("root"), Some("root")),
        )?;
    }

    Ok(ActivationOutcome {
        definition_changed: request.definition_changed,
        action,
        recovered_pending_activation: pending && !request.definition_changed,
    })
}

#[cfg(test)]
#[path = "activation/tests.rs"]
mod tests;
