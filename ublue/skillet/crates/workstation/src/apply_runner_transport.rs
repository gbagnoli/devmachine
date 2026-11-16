//! Run declared oneshot apply consumers with one verified temporary executable.
use crate::deployment::{activate_with_runner, TemporaryRunner};
use skillet_vm::{
    transport::{GuestCommand, GuestTransport},
    Error,
};
use std::{
    os::unix::process::ExitStatusExt as _,
    path::Path,
    process::{ExitStatus, Output},
};

/// All effects still traverse the caller's ownership-checked transport. Only
/// explicitly declared apply-unit starts gain a bounded runtime override.
pub struct ApplyRunnerTransport<'a, T> {
    guest: &'a T,
    runner: &'a TemporaryRunner,
}

impl<'a, T: GuestTransport> ApplyRunnerTransport<'a, T> {
    pub fn new(guest: &'a T, runner: &'a TemporaryRunner) -> Self {
        Self { guest, runner }
    }

    fn activate(&self, unit: &str) -> skillet_vm::Result<()> {
        let arguments = crate::host_apply::temporary_arguments(unit)
            .ok_or_else(|| Error::Invalid("undeclared temporary apply unit".into()))?;
        activate_with_runner(
            self.guest,
            unit,
            self.runner.path(),
            self.runner.token(),
            arguments,
        )
        .map_err(|error| Error::Invalid(error.to_string()))
    }
}

impl<T: GuestTransport> GuestTransport for ApplyRunnerTransport<'_, T> {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        // Credential installers can start their consumer inside the guest. Stage
        // that input first, then activate through the same current-code boundary.
        if let ("sudo" | "/usr/bin/sudo", ["-n", binary, "credential", "install", _, unit]) =
            (command.program, command.arguments)
        {
            if *binary == format!("/var/usrlocal/bin/skillet-{}", self.runner.host())
                && crate::host_apply::temporary_arguments(unit).is_some()
            {
                if input.is_none_or(<[u8]>::is_empty) {
                    return Err(Error::Invalid(
                        "credential installation requires payload stdin".into(),
                    ));
                }
                let mut deferred = command.arguments.to_vec();
                deferred.push("--no-start");
                let output = self.guest.execute(
                    &GuestCommand {
                        program: command.program,
                        arguments: &deferred,
                    },
                    input,
                )?;
                if output.status.success() {
                    self.activate(unit)?;
                }
                return Ok(output);
            }
        }
        let unit = match (command.program, command.arguments) {
            ("sudo" | "/usr/bin/sudo", ["-n", "systemctl", "start", unit])
            | ("systemctl", ["start", unit]) => Some(*unit),
            _ => None,
        };
        if let Some(unit) =
            unit.filter(|unit| crate::host_apply::temporary_arguments(unit).is_some())
        {
            if input.is_some() {
                return Err(Error::Invalid(
                    "apply unit activation must not consume payload stdin".into(),
                ));
            }
            self.activate(unit)?;
            return Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        }
        self.guest.execute(command, input)
    }
    fn upload(&self, source: &Path, destination: &str) -> skillet_vm::Result<()> {
        self.guest.upload(source, destination)
    }
}
