//! Native guest adapter for Syncthing's offline upstream generator.
use crate::CliCommonError;
use sha2::{Digest, Sha256};
use skillet_core::{
    credentials::CredentialManager,
    files::{FileMutationResource, FileReadResource, LocalFileResource, OwnerIdentity, Ownership},
    system::{AccountLookupResource, LinuxSystemResource},
};
use skillet_hosts::ServiceConfig;
use skillet_syncthing::{
    initial::{self, Generator, Settings},
    SyncthingConfig,
};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Seed a missing config; preserve every existing application setting
    Initialize {
        #[arg(long)]
        host_file: PathBuf,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let Commands::Initialize { host_file } = command;
    let host = std::fs::read_to_string(host_file)?;
    let profile = skillet_hosts::profile_for_name(host.trim())
        .ok_or_else(|| CliCommonError::Config("unknown Syncthing host".into()))?;
    let (config, protected) = declaration(&profile)?;
    let input = CredentialManager::new()?.read_secret(initial::CREDENTIAL)?;
    let settings = Settings::parse(input.as_bytes()).map_err(config_error)?;
    if settings.host != profile.id.as_str()
        || settings.runtime_hostname != std::fs::read_to_string("/etc/hostname")?.trim()
    {
        return Err(config_error(initial::Error::Binding));
    }
    let system = LinuxSystemResource::new();
    let files = LocalFileResource;
    let user = system
        .user_by_name(&config.data_owner)?
        .ok_or_else(|| CliCommonError::Config("Syncthing owner missing".into()))?;
    let group = system
        .group_by_name(&config.data_group)?
        .ok_or_else(|| CliCommonError::Config("Syncthing group missing".into()))?;
    if user.primary_gid != group.gid {
        return Err(CliCommonError::Config(
            "Syncthing primary group mismatch".into(),
        ));
    }
    let _lock = bootstrap_lock(&files)?;
    let root = Path::new(&config.data_path).join("config");
    if files
        .read_file(&root.join("config.xml"))
        .map_err(|e| CliCommonError::Config(e.to_string()))?
        .is_some()
    {
        initial::cleanup_completed(&files, &root, &settings).map_err(config_error)?;
        return Ok(());
    }
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "syncthing.service"])
        .status()?
        .success()
    {
        return Err(CliCommonError::Config(
            "stop Syncthing before initializing missing configuration".into(),
        ));
    }
    skillet_syncthing::prepare_storage(&files, &config)
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    let ownership = Ownership {
        uid: Some(OwnerIdentity::Id(user.uid)),
        gid: Some(OwnerIdentity::Id(group.gid)),
    };
    files
        .ensure_directory(&root, Some(0o700), &ownership)
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    let directives = skillet_podman::process_identity_directives(
        &system,
        &files,
        &skillet_podman::ProcessIdentity::Numeric {
            uid: config.container_uid,
            gid: config.container_gid,
        },
        Some(&skillet_podman::UserNamespaceMapping {
            host_user: skillet_podman::HostUser::Name(config.data_owner.clone()),
        }),
    )
    .map_err(|e| CliCommonError::Config(e.to_string()))?;
    let runtime = OfflineGenerator {
        directives,
        home: skillet_syncthing::container_home(&config)
            .map_err(|e| CliCommonError::Config(e.to_string()))?,
    };
    initial::initialize(&files, &runtime, &root, &settings, protected, &ownership)
        .map_err(config_error)?;
    Ok(())
}

fn declaration(
    profile: &skillet_hosts::HostProfile,
) -> Result<(SyncthingConfig, bool), CliCommonError> {
    let service = profile
        .services
        .iter()
        .find(|s| s.name() == "syncthing")
        .ok_or_else(|| CliCommonError::Config("host does not select Syncthing".into()))?;
    let ServiceConfig::Syncthing {
        files_path,
        data_path,
        data_owner,
        data_group,
        container_uid,
        container_gid,
    } = service.config
    else {
        return Err(CliCommonError::Config(
            "invalid Syncthing declaration".into(),
        ));
    };
    Ok((
        SyncthingConfig {
            files_path: files_path.into(),
            data_path: data_path.into(),
            data_owner: data_owner.into(),
            data_group: data_group.into(),
            container_uid,
            container_gid,
            network_name: String::new(),
        },
        service.ui.is_some_and(|ui| ui.tailnet_identity),
    ))
}

fn bootstrap_lock(files: &LocalFileResource) -> Result<std::fs::File, CliCommonError> {
    files
        .ensure_directory(
            Path::new("/var/lib/skillet"),
            Some(0o700),
            &Ownership::default(),
        )
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("/var/lib/skillet/syncthing-initial.lock")?;
    lock.lock()?;
    Ok(lock)
}

fn config_error(error: initial::Error) -> CliCommonError {
    CliCommonError::SyncthingInitial(error)
}

struct OfflineGenerator {
    directives: Vec<String>,
    home: String,
}
impl Generator for OfflineGenerator {
    fn recover(&self, staging: &Path) -> Result<(), initial::Error> {
        let owner = format!(
            "{:x}",
            Sha256::digest(staging.as_os_str().as_encoded_bytes())
        );
        cleanup_generator(&staging.join("generator.cid"), &owner)
    }
    fn generate(&self, staging: &Path) -> Result<(), initial::Error> {
        let owner = format!(
            "{:x}",
            Sha256::digest(staging.as_os_str().as_encoded_bytes())
        );
        let cid_path = staging.join("generator.cid");
        cleanup_generator(&cid_path, &owner)?;
        let mut command = Command::new("podman");
        command.arg("--log-level=error");

        command.args([
            "run",
            "--rm",
            "--network",
            "none",
            "--entrypoint",
            "/bin/syncthing",
        ]);
        command
            .arg("--cidfile")
            .arg(&cid_path)
            .arg("--label")
            .arg(format!("io.skillet.syncthing-initial={owner}"))
            .args(["--timeout", "90"]);
        for directive in &self.directives {
            let (key, value) = directive.split_once('=').ok_or(initial::Error::Generator)?;
            let flag = match key {
                "User" => "--user",
                "UIDMap" => "--uidmap",
                "GIDMap" => "--gidmap",
                _ => return Err(initial::Error::Generator),
            };
            command.args([flag, value]);
        }
        command.arg("--env").arg(format!("HOME={}", self.home));
        command
            .arg("--volume")
            .arg(format!("{}:/var/syncthing/config:z", staging.display()));
        command.args([
            initial::IMAGE,
            "generate",
            "--home",
            "/var/syncthing/config",
        ]);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|_| initial::Error::Generator)?;
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Some(status) = child.try_wait().map_err(|_| initial::Error::Generator)? {
                cleanup_generator(&cid_path, &owner)?;
                if !status.success() {
                    return Err(initial::Error::Generator);
                }
                for name in ["cert.pem", "key.pem", "config.xml"] {
                    std::fs::File::open(staging.join(name))
                        .and_then(|f| f.sync_all())
                        .map_err(|_| initial::Error::Generator)?;
                }
                std::fs::File::open(staging)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| initial::Error::Generator)?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                cleanup_generator(&cid_path, &owner)?;
                return Err(initial::Error::Generator);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn cleanup_generator(path: &Path, owner: &str) -> Result<(), initial::Error> {
    let cid = match std::fs::read_to_string(path) {
        Ok(cid) => cid,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(initial::Error::Generator),
    };
    let cid = cid.trim();
    if cid.len() != 64 || !cid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(initial::Error::Generator);
    }
    let exists = Command::new("podman")
        .args(["container", "exists", cid])
        .status()
        .map_err(|_| initial::Error::Generator)?;
    if exists.success() {
        let observed = Command::new("podman")
            .args([
                "inspect",
                "--format",
                "{{index .Config.Labels \"io.skillet.syncthing-initial\"}}",
                cid,
            ])
            .output()
            .map_err(|_| initial::Error::Generator)?;
        if !observed.status.success() || String::from_utf8_lossy(&observed.stdout).trim() != owner {
            return Err(initial::Error::Generator);
        }
        if !Command::new("podman")
            .args(["rm", "--force", cid])
            .stdout(Stdio::null())
            .status()
            .map_err(|_| initial::Error::Generator)?
            .success()
        {
            return Err(initial::Error::Generator);
        }
    } else if exists.code() != Some(1) {
        return Err(initial::Error::Generator);
    }
    std::fs::remove_file(path).map_err(|_| initial::Error::Generator)?;
    Ok(())
}
