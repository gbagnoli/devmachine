//! Explicit, one-time transition from bootstrap to interactive administration.
use crate::CliCommonError;
use nix::{
    fcntl::{Flock, FlockArg},
    unistd::Uid,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(clap::Subcommand, Debug)]
pub enum AccessCommands {
    /// Finish provisioning: require a sudo password and change password on next login
    Finalize {
        /// Existing administrator account to finalize
        #[arg(long)]
        user: String,
    },
}

pub fn dispatch(command: AccessCommands) -> Result<(), CliCommonError> {
    match command {
        AccessCommands::Finalize { user } => {
            validate_user(&user)?;
            if !Uid::effective().is_root() {
                return Err(config(
                    "access finalization requires root; run it with sudo",
                ));
            }
            let root = Path::new("/var/lib/skillet/access");
            fs::create_dir_all(root)?;
            let lock_file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(root.join("finalize.lock"))?;
            let _lock = Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)
                .map_err(|_| config("another access finalization is running"))?;
            let runtime = LinuxAccess {
                root: root.to_path_buf(),
            };
            finalize(&runtime, &user)?;
            tracing::info!(user, "Access finalized; sudo requires the account password");
            Ok(())
        }
    }
}

fn config(message: &str) -> CliCommonError {
    CliCommonError::Config(message.to_string())
}

fn validate_user(user: &str) -> Result<(), CliCommonError> {
    if user == "root"
        || user.len() > 32
        || user.is_empty()
        || !user.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_lowercase() || b == b'_' || (i > 0 && (b.is_ascii_digit() || b == b'-'))
        })
    {
        return Err(config("finalization requires a non-root Unix account name"));
    }
    Ok(())
}

struct PasswordState {
    fingerprint: String,
    expired: bool,
}

// A single adapter owns the account observation and the matching mutation.
trait AccessRuntime {
    fn password(&self, user: &str) -> Result<PasswordState, CliCommonError>;
    fn read_state(&self, user: &str) -> Result<Option<String>, CliCommonError>;
    fn write_state(&self, user: &str, state: &str) -> Result<(), CliCommonError>;
    fn require_sudo_password(&self, user: &str) -> Result<(), CliCommonError>;
    fn expire_password(&self, user: &str) -> Result<(), CliCommonError>;
}

fn finalize(runtime: &impl AccessRuntime, user: &str) -> Result<(), CliCommonError> {
    validate_user(user)?;
    let password = runtime.password(user)?;
    let state = runtime.read_state(user)?;
    // Re-converge durable sudo policy without repeating a one-time expiry.
    if state.as_deref() == Some("complete\n") {
        return runtime.require_sudo_password(user);
    }
    let original = if let Some(state) = &state {
        state
            .strip_prefix("pending:")
            .and_then(|value| value.strip_suffix('\n'))
            .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| config("invalid access finalization journal"))?
            .to_string()
    } else {
        runtime.write_state(user, &format!("pending:{}\n", password.fingerprint))?;
        password.fingerprint.clone()
    };
    runtime.require_sudo_password(user)?;
    // If interrupted after expiry or after an operator changed the password,
    // finish the journal without resetting the operator's new password.
    if !password.expired && password.fingerprint == original {
        runtime.expire_password(user)?;
    }
    runtime.write_state(user, "complete\n")
}

fn sudo_policy(user: &str) -> String {
    format!("# Managed by skillet access finalize\nDefaults:{user} !rootpw, !targetpw, !runaspw\n{user} ALL=(ALL:ALL) PASSWD: ALL\n")
}

struct LinuxAccess {
    root: PathBuf,
}

fn atomic_write(path: &Path, content: &[u8], mode: u32) -> Result<(), CliCommonError> {
    use std::os::unix::fs::PermissionsExt as _;
    let parent = path.parent().ok_or_else(|| config("file has no parent"))?;
    if fs::read(path).ok().as_deref() == Some(content)
        && fs::metadata(path)?.permissions().mode() & 0o777 == mode
    {
        return Ok(());
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| CliCommonError::Io(error.error))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn execute(program: &str, args: &[&str]) -> Result<(), CliCommonError> {
    let result = Command::new(program).args(args).output()?;
    if !result.status.success() {
        // Never include account database contents or command output in errors.
        return Err(config(&format!(
            "{program} failed during access finalization"
        )));
    }
    Ok(())
}

impl AccessRuntime for LinuxAccess {
    fn password(&self, user: &str) -> Result<PasswordState, CliCommonError> {
        let shadow = zeroize::Zeroizing::new(fs::read_to_string("/etc/shadow")?);
        let entry = shadow
            .lines()
            .find(|line| line.split(':').next() == Some(user))
            .ok_or_else(|| config("administrator account is absent from /etc/shadow"))?;
        let mut fields = entry.split(':');
        fields.next();
        let hash = fields
            .next()
            .ok_or_else(|| config("invalid password entry"))?;
        if hash.is_empty() || hash.starts_with(['!', '*']) {
            return Err(config(
                "administrator needs an unlocked bootstrap password before finalization",
            ));
        }
        let changed = fields
            .next()
            .ok_or_else(|| config("invalid password age entry"))?;
        Ok(PasswordState {
            fingerprint: format!("{:x}", Sha256::digest(hash.as_bytes())),
            expired: changed == "0",
        })
    }
    fn read_state(&self, user: &str) -> Result<Option<String>, CliCommonError> {
        match fs::read_to_string(self.root.join(user)) {
            Ok(state) => Ok(Some(state)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn write_state(&self, user: &str, state: &str) -> Result<(), CliCommonError> {
        atomic_write(&self.root.join(user), state.as_bytes(), 0o600)
    }
    fn require_sudo_password(&self, user: &str) -> Result<(), CliCommonError> {
        let directory = Path::new("/etc/sudoers.d");
        let name = format!("zz-skillet-access-{user}");
        for entry in fs::read_dir(directory)? {
            let filename = entry?.file_name().to_string_lossy().into_owned();
            if filename > name && !filename.contains('.') && !filename.ends_with('~') {
                return Err(config(
                    "a later sudoers rule exists; review sudo policy before finalizing",
                ));
            }
        }
        let mut candidate = tempfile::NamedTempFile::new_in(directory)?;
        candidate.write_all(sudo_policy(user).as_bytes())?;
        let candidate_path = candidate
            .path()
            .to_str()
            .ok_or_else(|| config("invalid sudoers path"))?;
        execute("/usr/sbin/visudo", &["-c", "-f", candidate_path])?;
        execute("/usr/sbin/visudo", &["-c"])?;
        atomic_write(&directory.join(name), sudo_policy(user).as_bytes(), 0o440)?;
        execute("/usr/sbin/visudo", &["-c"])
    }
    fn expire_password(&self, user: &str) -> Result<(), CliCommonError> {
        execute("/usr/bin/chage", &["-d", "0", user])
    }
}

#[cfg(test)]
#[path = "access/tests.rs"]
mod tests;
