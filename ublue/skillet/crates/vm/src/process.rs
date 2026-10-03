//! Bounded direct subprocess capture. Payloads and output stay in memory.
use crate::{Error, Result};
use std::{
    io::{Read, Write as _},
    os::unix::process::CommandExt as _,
    process::{Child, Command, Output, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: u64 = 64 * 1024 * 1024;

pub(crate) fn capture(command: Command, timeout: Duration) -> Result<Output> {
    capture_with_input(command, timeout, None)
}

pub(crate) fn capture_with_input(
    mut command: Command,
    timeout: Duration,
    input: Option<&[u8]>,
) -> Result<Output> {
    let child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut managed = ManagedChild {
        child,
        complete: false,
    };
    let stdout = read_stream(
        managed
            .child
            .stdout
            .take()
            .ok_or_else(|| Error::Invalid("tool stdout unavailable".into()))?,
    );
    let stderr = read_stream(
        managed
            .child
            .stderr
            .take()
            .ok_or_else(|| Error::Invalid("tool stderr unavailable".into()))?,
    );
    let writer = if let Some(input) = input {
        let mut pipe = managed
            .child
            .stdin
            .take()
            .ok_or_else(|| Error::Invalid("tool stdin unavailable".into()))?;
        let bytes = zeroize::Zeroizing::new(input.to_vec());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(pipe.write_all(&bytes));
        });
        Some(receiver)
    } else {
        None
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = managed.child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = receive(&stdout, deadline)??;
    let stderr = receive(&stderr, deadline)??;
    if let Some(writer) = writer {
        let written = receive(&writer, deadline)?;
        if status.success() {
            written?;
        }
    }
    managed.complete = true;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn read_stream(stream: impl Read + Send + 'static) -> Receiver<Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let mut bytes = Vec::new();
            stream.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
            if u64::try_from(bytes.len())
                .map_err(|_| Error::Invalid("tool output size overflow".into()))?
                > OUTPUT_LIMIT
            {
                return Err(Error::Invalid(
                    "tool output exceeded the memory limit".into(),
                ));
            }
            Ok(bytes)
        })();
        let _ = sender.send(result);
    });
    receiver
}

fn receive<T>(receiver: &Receiver<T>, deadline: Instant) -> Result<T> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => Error::Timeout,
            mpsc::RecvTimeoutError::Disconnected => {
                Error::Invalid("tool stream worker stopped".into())
            }
        })
}

struct ManagedChild {
    child: Child,
    complete: bool,
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.complete {
            if let Ok(id) = i32::try_from(self.child.id()) {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(id),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
#[path = "process/tests.rs"]
mod tests;
