//! Cached/interactive sudo uses a private askpass channel, never command payload stdin.
use skillet_vm::{
    artifacts,
    transport::{
        CredentialIdentity, GuestCommand, GuestTransport, ReverseUnixForward, SshTransport,
        UnixForward,
    },
    Error, Result,
};
use std::{
    cell::{OnceCell, RefCell},
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    process::Output,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
use zeroize::Zeroizing;

mod password_cache;
use crate::credential_cache::{CredentialCache, KernelCache};
use password_cache::{CacheBinding, PasswordState};

pub const SUDO_CACHE_LIFETIME: Duration = Duration::from_mins(30);
pub const SUDO_CACHE_NAMESPACE: &str = "sudo";

pub type PasswordPrompt = Arc<dyn Fn() -> std::io::Result<Zeroizing<String>> + Send + Sync>;

/// Workstation auth policy; VM automation continues to use its recorded transport.
pub struct SudoTransport<
    T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone = SshTransport,
> {
    ssh: T,
    workspace: PathBuf,
    prompt: Option<PasswordPrompt>,
    session: RefCell<Option<Session<T>>>,
    cache: Arc<dyn CredentialCache>,
    scope: OnceCell<Option<[u8; 32]>>,
}
impl<T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone> SudoTransport<T> {
    pub fn new(ssh: T, workspace: PathBuf, prompt: Option<PasswordPrompt>) -> Self {
        Self::with_cache(
            ssh,
            workspace,
            prompt,
            Arc::new(KernelCache::new(SUDO_CACHE_NAMESPACE)),
        )
    }
    pub fn with_cache(
        ssh: T,
        workspace: PathBuf,
        prompt: Option<PasswordPrompt>,
        cache: Arc<dyn CredentialCache>,
    ) -> Self {
        Self {
            ssh,
            workspace,
            prompt,
            session: RefCell::new(None),
            cache,
            scope: OnceCell::new(),
        }
    }
    fn binding(&self) -> Result<Option<CacheBinding>> {
        let current = self.ssh.credential_scope()?;
        if self
            .scope
            .get()
            .is_some_and(|previous| previous != &current)
        {
            if let Some(mut session) = self.session.borrow_mut().take() {
                session.cleanup_allowed = false;
                drop(session);
            }
            return Err(Error::Authentication("SSH target or host trust changed during this invocation; retry with the verified connection".into()));
        }
        let _ = self.scope.set(current);
        Ok(current.map(|scope| CacheBinding::new(Arc::clone(&self.cache), scope)))
    }
}
impl<T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone> GuestTransport
    for SudoTransport<T>
{
    fn upload(&self, source: &Path, destination: &str) -> Result<()> {
        self.ssh.upload(source, destination)
    }
    fn execute(&self, command: &GuestCommand<'_>, input: Option<&[u8]>) -> Result<Output> {
        if !matches!(command.program, "sudo" | "/usr/bin/sudo")
            || command.arguments.first() != Some(&"-n")
        {
            return self.ssh.execute(command, input);
        }
        let binding = self.binding()?;
        if self.prompt.is_none()
            && binding
                .as_ref()
                .is_none_or(|binding| binding.read().is_none())
        {
            let output = self.ssh.execute(command, input)?;
            if !output.status.success()
                && String::from_utf8_lossy(&output.stderr).contains("a password is required")
            {
                return Err(Error::Authentication("remote sudo needs a password; no usable cached credential; rerun in an interactive terminal (SSH key unlock is separate)".into()));
            }
            return Ok(output);
        }
        let mut session = self.session.borrow_mut();
        if session.is_none() {
            *session = Some(Session::start(
                &self.ssh,
                &self.workspace,
                self.prompt.clone(),
                binding,
            )?);
        }
        let session = session
            .as_ref()
            .ok_or_else(|| Error::Invalid("sudo session unavailable".into()))?;
        let operation = session
            .broker
            .operation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        let askpass = format!("SUDO_ASKPASS={}/helper", session.directory);
        let mut args = vec![askpass.as_str(), command.program, "-A"];
        args.extend_from_slice(&command.arguments[1..]);
        // env carries only a helper path, never the password. stdin is unchanged.
        let result = self.ssh.execute(
            &GuestCommand {
                program: "/usr/bin/env",
                arguments: &args,
            },
            input,
        );
        session.broker.complete(
            operation,
            result.as_ref().is_ok_and(|output| output.status.success()),
        )?;
        let error = session
            .broker
            .error
            .lock()
            .map_err(|_| Error::Invalid("sudo prompt state poisoned".into()))?
            .take();
        if let Some(error) = error {
            return Err(Error::Authentication(error));
        }
        result
    }
}

fn valid_directory(path: &str) -> bool {
    path.strip_prefix("/var/tmp/skillet-sudo-")
        .is_some_and(|suffix| {
            suffix.len() == 12 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn checked(ssh: &impl GuestTransport, program: &str, args: &[&str]) -> Result<Output> {
    let output = ssh.execute(
        &GuestCommand {
            program,
            arguments: args,
        },
        None,
    )?;
    if !output.status.success() {
        return Err(Error::Guest {
            operation: "preparing/cleaning sudo askpass channel".into(),
            code: output.status.code(),
        });
    }
    Ok(output)
}

struct Session<T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone> {
    ssh: T,
    directory: String,
    forward: Option<UnixForward>,
    broker: Broker,
    cleanup_allowed: bool,
}
impl<T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone> Session<T> {
    fn start(
        ssh: &T,
        workspace: &Path,
        prompt: Option<PasswordPrompt>,
        binding: Option<CacheBinding>,
    ) -> Result<Self> {
        let artifacts = artifacts::build(&artifacts::BuildRequest {
            workspace,
            packages: &["skillet-sudo-askpass"],
            target: "x86_64-unknown-linux-musl",
            profile: "release",
        })?;
        let helper = artifacts
            .get("skillet-sudo-askpass")
            .ok_or_else(|| Error::Invalid("sudo helper artifact absent".into()))?;
        let broker = Broker::start(prompt, binding)?;
        let output = checked(ssh, "mktemp", &["-d", "/var/tmp/skillet-sudo-XXXXXXXXXXXX"])?;
        let directory = String::from_utf8(output.stdout)
            .map_err(|_| Error::Invalid("invalid sudo staging path".into()))?
            .trim()
            .to_owned();
        if !valid_directory(&directory) {
            return Err(Error::Invalid("invalid sudo staging path".into()));
        }
        let cleanup_ssh = ssh.clone();

        let mut session = Self {
            ssh: cleanup_ssh,
            directory,
            forward: None,
            broker,
            cleanup_allowed: true,
        };
        let remote_helper = format!("{}/helper", session.directory);
        ssh.upload(helper, &remote_helper)?;
        checked(ssh, "chmod", &["0700", &remote_helper])?;
        let expected = skillet_vm::delivery::sha256(helper)?;
        let digest = checked(ssh, "sha256sum", &[&remote_helper])?;
        if String::from_utf8_lossy(&digest.stdout)
            .split_whitespace()
            .next()
            != Some(expected.as_str())
        {
            return Err(Error::Invalid("sudo helper hash mismatch".into()));
        }
        session.forward = Some(ssh.reverse_unix_forward(
            &format!("{}/askpass.sock", session.directory),
            &session.broker.socket,
        )?);
        Ok(session)
    }
}
impl<T: GuestTransport + ReverseUnixForward + CredentialIdentity + Clone> Drop for Session<T> {
    fn drop(&mut self) {
        self.forward.take();
        self.broker.shutdown();
        if !self.cleanup_allowed {
            eprintln!("SSH identity changed; retained former target's private sudo helper directory {}; no password was stored there", self.directory);
            return;
        }
        let helper = format!("{}/helper", self.directory);
        let socket = format!("{}/askpass.sock", self.directory);
        if checked(&self.ssh, "rm", &["-f", &helper, &socket])
            .and_then(|_| checked(&self.ssh, "rmdir", &[&self.directory]))
            .is_err()
        {
            eprintln!("Warning: could not remove private sudo helper directory {}; no password was stored there", self.directory);
        }
    }
}

struct Broker {
    _directory: tempfile::TempDir,
    socket: PathBuf,
    operation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    worker: Option<JoinHandle<()>>,
    state: Arc<Mutex<PasswordState>>,
    binding: Option<CacheBinding>,
}
impl Broker {
    fn start(prompt: Option<PasswordPrompt>, binding: Option<CacheBinding>) -> Result<Self> {
        let directory = tempfile::Builder::new().prefix("skillet-sudo-").tempdir()?;
        let socket = directory.path().join("askpass.sock");
        let listener = UnixListener::bind(&socket)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let operation = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let worker_operation = Arc::clone(&operation);
        let worker_stop = Arc::clone(&stop);
        let worker_error = Arc::clone(&error);
        let state = Arc::new(Mutex::new(PasswordState::default()));
        let worker_state = Arc::clone(&state);
        let worker_binding = binding.clone();
        let worker = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::SeqCst) {
                if let Ok(mut state) = worker_state.lock() {
                    state.expire();
                }
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    Err(_) => break,
                };
                let result = (|| -> std::io::Result<()> {
                    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                    let mut request = [0; 24];
                    stream.read_exact(&mut request)?;
                    if &request != b"skillet-sudo-askpass-v1\n" {
                        return Ok(());
                    }
                    let current = worker_operation.load(Ordering::SeqCst);
                    let mut state = worker_state
                        .lock()
                        .map_err(|_| std::io::Error::other("sudo credential state poisoned"))?;
                    let password =
                        state.offer(current, prompt.as_ref(), worker_binding.as_ref())?;
                    stream.write_all(password.as_bytes())?;
                    stream.write_all(b"\n")?;
                    state.sent();
                    Ok(())
                })();
                if result.is_err() {
                    if let Ok(mut error) = worker_error.lock() {
                        *error = Some(if prompt.is_none() {
                            "cached sudo credential expired or was rejected; rerun in an interactive terminal".into()
                        } else {
                            "sudo password prompt/channel failed; no password or credential payload was logged".into()
                        });
                    }
                }
            }
        });
        Ok(Self {
            _directory: directory,
            socket,
            operation,
            stop,
            error,
            worker: Some(worker),
            state,
            binding,
        })
    }
}
impl Broker {
    fn complete(&self, operation: u64, success: bool) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| Error::Invalid("sudo credential state poisoned".into()))?
            .complete(operation, success, self.binding.as_ref());
        Ok(())
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "sudo/tests.rs"]
mod tests;

impl Drop for Broker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
