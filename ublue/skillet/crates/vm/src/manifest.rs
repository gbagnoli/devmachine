use crate::{Error, Result};
use nix::fcntl::{Flock, FlockArg};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunIdentity {
    host: String,
    instance: String,
}

impl RunIdentity {
    pub fn new(host: &str, instance: &str) -> Result<Self> {
        if !valid_label(host, false) || !valid_label(instance, true) {
            return Err(Error::Invalid(
                "host or instance contains invalid characters".into(),
            ));
        }
        Ok(Self {
            host: host.into(),
            instance: instance.into(),
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn instance(&self) -> &str {
        &self.instance
    }
    pub fn domain_name(&self) -> String {
        format!("{}-test-{}", self.host, self.instance)
    }
}

fn valid_label(value: &str, digit_first: bool) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || (digit_first && byte.is_ascii_digit()))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Native,
    Flatpak,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Test,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub backend: Backend,
    pub uri: String,
    pub runtime_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshTarget {
    pub user: String,
    pub address: String,
    pub port: u16,
    pub identity: PathBuf,
    pub known_hosts: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Preparing,
    Defined,
    Started,
    Ready,
    ExternalCleanupComplete,
    DomainRemoved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactHashes {
    pub host: String,
    pub generic: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmRun {
    pub version: u32,
    pub owner_uid: u32,
    pub identity: RunIdentity,
    pub environment: Environment,
    pub guest_hostname: String,
    pub uuid: Uuid,
    pub connection: Connection,
    pub ssh: SshTarget,
    pub disk: PathBuf,
    pub ignition: PathBuf,
    pub source_commit: String,
    pub captured: Option<ArtifactHashes>,
    pub deployed: Option<ArtifactHashes>,
    pub phase: Phase,
}

/// The manifest contains only ownership/configuration, never secret payloads.
/// External provider journals remain separate and are preserved during import.
pub struct ManifestStore {
    root: PathBuf,
    owner_uid: u32,
}

/// Held across an entire lifecycle mutation, including its external calls.
pub struct RunLock {
    _lock: Flock<File>,
}

impl ManifestStore {
    pub fn new(root: &Path, owner_uid: u32) -> Result<Self> {
        if !root.is_absolute()
            || root
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(Error::Invalid("artifact root must be absolute".into()));
        }
        reject_symlinks(root)?;
        Ok(Self {
            root: root.into(),
            owner_uid,
        })
    }

    pub fn run_dir(&self, identity: &RunIdentity) -> PathBuf {
        self.root.join(identity.domain_name())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create or resume a creation intent before any guest resources are
    /// defined. Matching Preparing and Defined states can be recovered;
    /// ambiguous or progressed runs must be inspected explicitly.
    pub fn prepare_intent(
        &self,
        identity: &RunIdentity,
        connection: Connection,
        ssh_port: u16,
        source_commit: &str,
    ) -> Result<VmRun> {
        if !(2200..=2299).contains(&ssh_port) || source_commit.trim().is_empty() {
            return Err(Error::Invalid(
                "invalid SSH port or empty source revision for VM creation".into(),
            ));
        }
        connection.validate(self.owner_uid)?;
        let dir = self.run_dir(identity);
        reject_symlinks(&self.root)?;
        if !self.root.exists() {
            fs::create_dir_all(&self.root)?;
        }
        reject_symlinks(&self.root)?;
        let root_metadata = fs::metadata(&self.root)?;
        if !root_metadata.is_dir() || root_metadata.uid() != self.owner_uid {
            return Err(Error::Invalid(
                "VM artifact root is not a directory owned by the current user".into(),
            ));
        }
        if dir.exists() {
            let existing = self.load(identity)?;
            if matches!(existing.phase, Phase::Preparing | Phase::Defined)
                && existing.connection == connection
                && existing.ssh.port == ssh_port
                && existing.source_commit == source_commit
            {
                return Ok(existing);
            }
            return Err(Error::Invalid(
                "VM run already exists and is not a matching recoverable creation; inspect or dispose it before creating".into(),
            ));
        }
        fs::create_dir(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let run = VmRun {
            version: MANIFEST_VERSION,
            owner_uid: self.owner_uid,
            identity: identity.clone(),
            environment: Environment::Test,
            guest_hostname: identity.domain_name(),
            uuid: Uuid::new_v4(),
            connection,
            ssh: SshTarget {
                user: "giacomo".into(),
                address: "127.0.0.1".into(),
                port: ssh_port,
                identity: dir.join("ssh/id_ed25519"),
                known_hosts: dir.join("ssh/known_hosts"),
            },
            disk: dir.join(format!("{}.qcow2", identity.domain_name())),
            ignition: dir
                .join("ignition")
                .join(format!("{}.ign", identity.host())),
            source_commit: source_commit.into(),
            captured: None,
            deployed: None,
            phase: Phase::Preparing,
        };
        self.write(&run, false)?;
        Ok(run)
    }

    /// Record successful VM definition/start only after callers validate the
    /// runtime UUID and disk ownership. Hashes are read from the captured files.
    pub fn mark_started(&self, identity: &RunIdentity) -> Result<VmRun> {
        let mut run = self.load(identity)?;
        if !matches!(run.phase, Phase::Preparing | Phase::Defined) {
            return Err(Error::Invalid(
                "only a preparing or defined VM can be recorded as started".into(),
            ));
        }
        let dir = self.run_dir(identity);
        run.captured = Some(ArtifactHashes {
            host: read_hash(&dir.join("skillet.sha256"))?,
            generic: read_hash(&dir.join("skillet-generic.sha256"))?,
        });
        run.phase = Phase::Started;
        self.save(&run)?;
        Ok(run)
    }

    pub fn mark_defined(&self, identity: &RunIdentity) -> Result<VmRun> {
        let mut run = self.load(identity)?;
        if run.phase != Phase::Preparing {
            return Err(Error::Invalid(
                "only a preparing VM can be recorded as defined".into(),
            ));
        }
        let dir = self.run_dir(identity);
        run.captured = Some(ArtifactHashes {
            host: read_hash(&dir.join("skillet.sha256"))?,
            generic: read_hash(&dir.join("skillet-generic.sha256"))?,
        });
        run.phase = Phase::Defined;
        self.save(&run)?;
        Ok(run)
    }

    /// Compatibility entry points obtain identity from recorded fields, never
    /// by splitting a potentially ambiguous composite directory name.
    pub fn load_directory(&self, dir: &Path) -> Result<VmRun> {
        if dir.parent() != Some(self.root.as_path()) {
            return Err(Error::Invalid(
                "run is outside the recorded artifact root".into(),
            ));
        }
        self.validate_directory(dir)?;
        let json = dir.join("vm.json");
        reject_symlinks(&json)?;
        let identity = if json.exists() {
            let run: VmRun = serde_json::from_str(&read_file(&json)?)?;
            run.identity
        } else {
            let config = parse_legacy(&read_file(&dir.join("run.conf"))?)?;
            let host = config
                .get("host")
                .ok_or_else(|| Error::Invalid("legacy manifest lacks host".into()))?;
            let vm = config
                .get("vm")
                .ok_or_else(|| Error::Invalid("legacy manifest lacks vm".into()))?;
            let instance = vm
                .strip_prefix(&format!("{host}-test-"))
                .ok_or_else(|| Error::Invalid("legacy VM does not match its profile".into()))?;
            RunIdentity::new(host, instance)?
        };
        if self.run_dir(&identity) != dir {
            return Err(Error::Invalid(
                "recorded identity differs from run directory".into(),
            ));
        }
        self.load(&identity)
    }

    pub fn lock(&self, identity: &RunIdentity) -> Result<RunLock> {
        let dir = self.run_dir(identity);
        self.validate_directory(&dir)?;
        let path = dir.join(".vm.lock");
        reject_symlinks(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path)?;
        if !file.metadata()?.is_file() || file.metadata()?.uid() != self.owner_uid {
            return Err(Error::Invalid(
                "invalid VM lock file ownership or type".into(),
            ));
        }
        let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
            if error == nix::errno::Errno::EWOULDBLOCK {
                Error::Busy
            } else {
                Error::Io(std::io::Error::from_raw_os_error(error as i32))
            }
        })?;
        Ok(RunLock { _lock: lock })
    }

    /// Read without migrating or contacting a runtime; safe for retained runs.
    pub fn load(&self, identity: &RunIdentity) -> Result<VmRun> {
        let dir = self.run_dir(identity);
        self.validate_directory(&dir)?;
        let json = dir.join("vm.json");
        reject_symlinks(&json)?;
        let run = if json.exists() {
            serde_json::from_str(&read_file(&json)?)?
        } else {
            self.read_legacy(identity, &dir)?
        };
        self.validate(&run, identity)?;
        Ok(run)
    }

    /// Explicit, atomic import. Keep all legacy files and provider journals.
    pub fn import(&self, identity: &RunIdentity) -> Result<VmRun> {
        let run = self.load(identity)?;
        let path = self.run_dir(identity).join("vm.json");
        if !path.exists() {
            match self.write(&run, false) {
                Ok(()) => (),
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return self.load(identity);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(run)
    }

    pub fn save(&self, run: &VmRun) -> Result<()> {
        self.write(run, true)
    }

    fn write(&self, run: &VmRun, replace: bool) -> Result<()> {
        self.validate(run, &run.identity)?;
        let dir = self.run_dir(&run.identity);
        self.validate_directory(&dir)?;
        let path = dir.join("vm.json");
        reject_symlinks(&path)?;
        let mut file = tempfile::NamedTempFile::new_in(&dir)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(&serde_json::to_vec_pretty(run)?)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        if replace {
            file.persist(&path)
                .map_err(|error| Error::Io(error.error))?;
        } else {
            file.persist_noclobber(&path)
                .map_err(|error| Error::Io(error.error))?;
        }
        File::open(dir)?.sync_all()?;
        Ok(())
    }

    fn validate_directory(&self, dir: &Path) -> Result<()> {
        reject_symlinks(dir)?;
        let metadata = fs::metadata(dir)?;
        if !metadata.is_dir() || metadata.uid() != self.owner_uid {
            return Err(Error::Invalid(
                "run directory is not owned by the current user".into(),
            ));
        }
        Ok(())
    }

    pub fn validate(&self, run: &VmRun, identity: &RunIdentity) -> Result<()> {
        RunIdentity::new(run.identity.host(), run.identity.instance())?;
        if run.version != MANIFEST_VERSION
            || run.owner_uid != self.owner_uid
            || &run.identity != identity
            || run.guest_hostname != identity.domain_name()
            || run.uuid.is_nil()
        {
            return Err(Error::Invalid(
                "manifest version, identity or owner mismatch".into(),
            ));
        }
        let dir = self.run_dir(identity);
        if run.disk != dir.join(format!("{}.qcow2", identity.domain_name()))
            || run.ignition
                != dir
                    .join("ignition")
                    .join(format!("{}.ign", identity.host()))
            || run.ssh.identity != dir.join("ssh/id_ed25519")
            || run.ssh.known_hosts != dir.join("ssh/known_hosts")
            || run.ssh.user != "giacomo"
            || run.ssh.address != "127.0.0.1"
            || !(2200..=2299).contains(&run.ssh.port)
        {
            return Err(Error::Invalid(
                "manifest artifact paths or SSH target mismatch".into(),
            ));
        }
        run.connection.validate(self.owner_uid)?;
        if run.phase != Phase::Preparing && run.captured.is_none() {
            return Err(Error::Invalid(
                "prepared runs require captured artifact hashes".into(),
            ));
        }
        for path in [
            &run.disk,
            &run.ignition,
            &run.ssh.identity,
            &run.ssh.known_hosts,
        ] {
            reject_symlinks(path)?;
        }
        for hashes in [&run.captured, &run.deployed].into_iter().flatten() {
            if !valid_hash(&hashes.host) || !valid_hash(&hashes.generic) {
                return Err(Error::Invalid("artifact hashes must be SHA256".into()));
            }
        }
        Ok(())
    }

    fn read_legacy(&self, identity: &RunIdentity, dir: &Path) -> Result<VmRun> {
        let config = parse_legacy(&read_file(&dir.join("run.conf"))?)?;
        let field = |name: &str| {
            config
                .get(name)
                .cloned()
                .ok_or_else(|| Error::Invalid(format!("legacy manifest lacks {name}")))
        };
        if field("vm")? != identity.domain_name() || field("host")? != identity.host() {
            return Err(Error::Invalid("legacy manifest identity mismatch".into()));
        }
        let backend = match config.get("backend").map(String::as_str) {
            None | Some("" | "native") => Backend::Native,
            Some("flatpak") => Backend::Flatpak,
            _ => return Err(Error::Invalid("unknown legacy VM backend".into())),
        };
        let uuid = Uuid::parse_str(read_file(&dir.join("domain.uuid"))?.trim()).map_err(|_| {
            Error::Invalid(
                "legacy UUID is absent or invalid; inspect partial creation before adoption".into(),
            )
        })?;
        let captured = Some(ArtifactHashes {
            host: read_hash(&dir.join("skillet.sha256"))?,
            generic: read_hash(&dir.join("skillet-generic.sha256"))?,
        });
        let host_deployed = dir.join("deployed-skillet.sha256");
        let generic_deployed = dir.join("deployed-skillet-generic.sha256");
        let deployed = match (host_deployed.exists(), generic_deployed.exists()) {
            (false, false) => None,
            (true, true) => Some(ArtifactHashes {
                host: read_hash(&host_deployed)?,
                generic: read_hash(&generic_deployed)?,
            }),
            _ => return Err(Error::Invalid("incomplete deployed artifact hashes".into())),
        };
        Ok(VmRun {
            version: MANIFEST_VERSION,
            owner_uid: self.owner_uid,
            identity: identity.clone(),
            environment: Environment::Test,
            guest_hostname: identity.domain_name(),
            uuid,
            connection: Connection {
                backend,
                uri: field("uri")?,
                runtime_dir: field("runtime_dir")?.into(),
            },
            ssh: SshTarget {
                user: "giacomo".into(),
                address: "127.0.0.1".into(),
                port: field("ssh_port")?
                    .parse()
                    .map_err(|_| Error::Invalid("invalid legacy SSH port".into()))?,
                identity: dir.join("ssh/id_ed25519"),
                known_hosts: dir.join("ssh/known_hosts"),
            },
            disk: dir.join(format!("{}.qcow2", identity.domain_name())),
            ignition: dir
                .join("ignition")
                .join(format!("{}.ign", identity.host())),
            source_commit: field("source_commit")?,
            captured,
            deployed,
            phase: Phase::Started,
        })
    }
}

impl Connection {
    pub fn validate(&self, owner_uid: u32) -> Result<()> {
        let base = PathBuf::from(format!("/run/user/{owner_uid}"));
        if self.uri != "qemu:///session"
            || !self.runtime_dir.is_absolute()
            || self.runtime_dir.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || (self.backend == Backend::Native && !self.runtime_dir.starts_with(&base))
            || (self.backend == Backend::Flatpak && self.runtime_dir != base.join("skvm"))
        {
            return Err(Error::Invalid(
                "unsupported libvirt connection or runtime path".into(),
            ));
        }
        reject_symlinks(&self.runtime_dir)
    }
}

fn parse_legacy(contents: &str) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for line in contents.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| Error::Invalid("malformed legacy manifest line".into()))?;
        if !matches!(
            key,
            "vm" | "host"
                | "uri"
                | "ssh_port"
                | "image"
                | "artifact"
                | "runtime_dir"
                | "backend"
                | "source_commit"
        ) || values.insert(key.into(), value.into()).is_some()
        {
            return Err(Error::Invalid(
                "unknown or duplicate legacy manifest field".into(),
            ));
        }
    }
    Ok(values)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn read_hash(path: &Path) -> Result<String> {
    let content = read_file(path)?;
    let hash = content
        .split_whitespace()
        .next()
        .ok_or_else(|| Error::Invalid("empty artifact hash file".into()))?;
    if !valid_hash(hash) {
        return Err(Error::Invalid("invalid artifact hash file".into()));
    }
    Ok(hash.to_ascii_lowercase())
}
fn read_file(path: &Path) -> Result<String> {
    reject_symlinks(path)?;
    if !fs::metadata(path)?.is_file() {
        return Err(Error::Invalid("VM metadata must be a regular file".into()));
    }
    Ok(fs::read_to_string(path)?)
}

/// Refuse links in ancestors as well as the final component; missing leaves are
/// allowed for preparation and interrupted cleanup, but never adopted as owned.
pub(crate) fn reject_symlinks(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Invalid(
                    "symlinked VM artifact or runtime path".into(),
                ))
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
pub(crate) mod tests;
