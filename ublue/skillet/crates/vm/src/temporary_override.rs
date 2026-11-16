//! Bounded runtime systemd `ExecStart` overrides for one-shot guest operations.
use crate::{
    transport::{GuestCommand, GuestTransport},
    Error, Result,
};
use std::process::Output;

/// Run an operation with a temporary `ExecStart` drop-in on a single systemd
/// unit. The unique drop-in is removed and systemd reloaded even when setup
/// verification or the operation fails.
pub fn with_exec_start<T>(
    guest: &impl GuestTransport,
    unit: &str,
    token: &str,
    executable: &str,
    arguments: &[&str],
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    with_exec_start_impl(guest, unit, token, executable, arguments, false, operation)
}

/// Stage a native helper override before its parent creates the unit fragment.
/// Only an explicitly observed not-found unit may defer command verification.
/// Existing units retain exact command verification and both paths retain cleanup.
pub fn with_deferred_exec_start<T>(
    guest: &impl GuestTransport,
    unit: &str,
    token: &str,
    executable: &str,
    arguments: &[&str],
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    with_exec_start_impl(guest, unit, token, executable, arguments, true, operation)
}

fn with_exec_start_impl<T>(
    guest: &impl GuestTransport,
    unit: &str,
    token: &str,
    executable: &str,
    arguments: &[&str],
    deferred: bool,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if !valid_unit(unit)
        || token.len() != 32
        || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !valid_executable(executable)
        || arguments.iter().any(|argument| {
            argument.is_empty()
                || !argument
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
        })
    {
        return Err(Error::Invalid("invalid temporary systemd override".into()));
    }
    let directory = format!("/run/systemd/system/{unit}.d");
    let dropin = format!("{directory}/zzzz-skillet-remote-apply-{token}.conf");
    let command = std::iter::once(executable)
        .chain(arguments.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    // ExecStopPost is the guest-side fail-safe: systemd removes the override
    // even if the SSH client exits while waiting for this oneshot service.
    let contents = format!(
        "[Service]\nExecStart=\nExecStart={command}\nExecStopPost=/usr/bin/rm -f {dropin}\nExecStopPost=/usr/bin/systemctl daemon-reload\n"
    );
    let setup = (|| -> Result<()> {
        run_sudo(
            guest,
            &[
                "install", "-d", "-o", "0", "-g", "0", "-m", "0755", &directory,
            ],
            None,
        )?;
        run_sudo(guest, &["tee", &dropin], Some(contents.as_bytes()))?;
        run_sudo(guest, &["restorecon", "-F", &dropin], None)?;
        run_sudo(guest, &["systemctl", "daemon-reload"], None)?;
        verify_effective(guest, unit, &command, deferred)?;
        Ok(())
    })();
    let result = setup.and_then(|()| operation());
    let cleanup = (|| -> Result<()> {
        run_sudo(guest, &["rm", "-f", &dropin], None)?;
        run_sudo(
            guest,
            &["rmdir", "--ignore-fail-on-non-empty", &directory],
            None,
        )?;
        run_sudo(guest, &["systemctl", "daemon-reload"], None)?;
        Ok(())
    })();
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(Error::Invalid(format!(
            "temporary override failed ({error}); cleanup also failed ({cleanup_error})"
        ))),
    }
}

fn verify_effective(
    guest: &impl GuestTransport,
    unit: &str,
    command: &str,
    deferred: bool,
) -> Result<()> {
    let effective = guest.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &[
                "-n",
                "systemctl",
                "show",
                "--property=ExecStart",
                "--value",
                unit,
            ],
        },
        None,
    )?;
    if effective.status.success() && String::from_utf8_lossy(&effective.stdout).contains(command) {
        return Ok(());
    }
    if deferred && matches!(effective.status.code(), Some(0 | 1 | 4)) {
        let state = guest.execute(
            &GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &[
                    "-n",
                    "systemctl",
                    "show",
                    "--property=LoadState",
                    "--value",
                    unit,
                ],
            },
            None,
        )?;
        if matches!(state.status.code(), Some(0 | 1 | 4))
            && state.stdout.strip_suffix(b"\n").unwrap_or(&state.stdout) == b"not-found"
        {
            return Ok(());
        }
    }
    Err(Error::Invalid(format!(
        "temporary executable command did not become the effective unit ExecStart: {unit}"
    )))
}

#[cfg(test)]
#[path = "temporary_override_tests.rs"]
mod tests;

fn valid_unit(unit: &str) -> bool {
    !unit.is_empty()
        && unit.ends_with(".service")
        && unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.@".contains(&byte))
}

fn valid_executable(path: &str) -> bool {
    (path == "/var/usrlocal/bin/skillet"
        || path.starts_with("/var/usrlocal/bin/.skillet-remote-apply-"))
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
}

fn run_sudo(
    guest: &impl GuestTransport,
    arguments: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut command = vec!["-n"];
    command.extend_from_slice(arguments);
    run_checked(guest, "/usr/bin/sudo", &command, input)
}

fn run_checked(
    guest: &impl GuestTransport,
    program: &str,
    arguments: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let output: Output = guest.execute(&GuestCommand { program, arguments }, input)?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr)
            .chars()
            .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
            .take(1024)
            .collect::<String>();
        return Err(Error::Invalid(format!(
            "remote operation {program} failed: {}",
            diagnostic.trim()
        )));
    }
    Ok(output.stdout)
}
