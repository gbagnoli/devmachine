//! Bounded direct subprocess capture; callers own their typed tool contracts.
use crate::{Error, Result};
use std::{
    fs,
    os::unix::process::CommandExt as _,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn capture(mut command: Command, timeout: Duration) -> Result<Output> {
    let stdout = tempfile::NamedTempFile::new()?;
    let stderr = tempfile::NamedTempFile::new()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.as_file().try_clone()?)
        .process_group(0)
        .spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            result => {
                if let Ok(id) = i32::try_from(child.id()) {
                    let _ = nix::sys::signal::killpg(
                        nix::unistd::Pid::from_raw(id),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
                let _ = child.kill();
                let _ = child.wait();
                return match result {
                    Err(error) => Err(error.into()),
                    _ => Err(Error::Timeout),
                };
            }
        }
    };
    Ok(Output {
        status,
        stdout: fs::read(stdout.path())?,
        stderr: fs::read(stderr.path())?,
    })
}
