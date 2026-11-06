//! Workstation-side selection and startup of the local libvirt runtime.
use crate::{
    backend::{ProcessExecutor, VirshBackend},
    Backend, Connection, Error, Result,
};
use std::{
    env, fs,
    fs::OpenOptions,
    os::unix::{
        fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
        process::CommandExt as _,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const URI: &str = "qemu:///session";
const READY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct SelectedRuntime {
    pub connection: Connection,
    pub virsh_wrapper: PathBuf,
    pub virt_install_wrapper: PathBuf,
}

/// Select a supported backend and make its user-session daemons available.
/// Native libvirt is preferred when its focused tools are present; otherwise
/// use the already installed Flatpak virt-manager and its QEMU extension.
pub fn prepare(butane_dir: &Path, owner_uid: u32) -> Result<SelectedRuntime> {
    validate_kvm_access()?;
    let user_runtime = standard_runtime_dir(owner_uid);
    validate_user_runtime_dir(&user_runtime, owner_uid)?;
    let virsh_wrapper = butane_dir.join("bin/virsh");
    let virt_install_wrapper = butane_dir.join("bin/virt-install");
    if native_tools_available() {
        let standard = native_connection(standard_runtime_dir(owner_uid));
        if ensure_connection(butane_dir, owner_uid, &standard).is_ok() {
            return Ok(SelectedRuntime {
                connection: standard,
                virsh_wrapper,
                virt_install_wrapper,
            });
        }

        let isolated_dir = standard_runtime_dir(owner_uid).join("skillet-test-libvirt");
        let isolated = native_connection(isolated_dir);
        ensure_connection(butane_dir, owner_uid, &isolated)?;
        return Ok(SelectedRuntime {
            connection: isolated,
            virsh_wrapper,
            virt_install_wrapper,
        });
    }

    let runtime_dir = standard_runtime_dir(owner_uid).join("skvm");
    let connection = Connection {
        backend: Backend::Flatpak,
        uri: URI.into(),
        runtime_dir: runtime_dir.clone(),
    };
    ensure_connection(butane_dir, owner_uid, &connection)?;
    Ok(SelectedRuntime {
        connection,
        virsh_wrapper,
        virt_install_wrapper,
    })
}

/// Check the same host virtualization prerequisite before image downloads or
/// artifact builds begin.
pub fn validate_kvm_access() -> Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .map_err(|error| Error::Invalid(format!("/dev/kvm is unavailable: {error}")))?;
    Ok(())
}

/// Start and verify the recorded backend for an existing run without changing
/// its selected URI or runtime path.
pub fn ensure_connection(butane_dir: &Path, owner_uid: u32, connection: &Connection) -> Result<()> {
    validate_user_runtime_dir(&standard_runtime_dir(owner_uid), owner_uid)?;
    connection.validate(owner_uid)?;
    let virsh_wrapper = butane_dir.join("bin/virsh");
    let backend = VirshBackend::new(
        connection.clone(),
        owner_uid,
        &virsh_wrapper,
        ProcessExecutor::default(),
    )?;
    match connection.backend {
        Backend::Native => {
            if !native_tools_available() {
                return Err(Error::Invalid(
                    "native libvirt tools are unavailable".into(),
                ));
            }
            let standard = standard_runtime_dir(owner_uid);
            let isolated = standard.join("skillet-test-libvirt");
            if connection.runtime_dir == standard {
                return backend.probe();
            }
            if connection.runtime_dir != isolated {
                return Err(Error::Invalid(
                    "native VM uses an unsupported runtime directory".into(),
                ));
            }
            ensure_private_runtime_dir(&isolated, owner_uid)?;
            start_native_daemons(&isolated)?;
            backend.probe()
        }
        Backend::Flatpak => {
            ensure_flatpak_installed()?;
            let runtime_dir = standard_runtime_dir(owner_uid).join("skvm");
            if connection.runtime_dir != runtime_dir {
                return Err(Error::Invalid(
                    "Flatpak VM uses an unsupported runtime directory".into(),
                ));
            }
            ensure_private_runtime_dir(&runtime_dir, owner_uid)?;
            let flatpak_runner = butane_dir.join("bin/flatpak-virt");
            let qemu_socket = runtime_socket(&runtime_dir, "virtqemud-sock");
            let storage_socket = runtime_socket(&runtime_dir, "virtstoraged-sock");
            let qemu_ready =
                fs::metadata(qemu_socket).is_ok_and(|metadata| metadata.file_type().is_socket());
            let storage_ready =
                fs::metadata(storage_socket).is_ok_and(|metadata| metadata.file_type().is_socket());
            match (qemu_ready, storage_ready) {
                (true, true) => backend.probe()?,
                (false, false) => {
                    start_flatpak_daemons(&runtime_dir, &flatpak_runner)?;
                    backend.probe()?;
                }
                _ => {
                    return Err(Error::Invalid(
                        "Flatpak libvirt runtime has only some expected sockets; inspect its retained log"
                            .into(),
                    ));
                }
            }
            Ok(())
        }
    }
}

fn standard_runtime_dir(owner_uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{owner_uid}"))
}

fn native_connection(runtime_dir: PathBuf) -> Connection {
    Connection {
        backend: Backend::Native,
        uri: URI.into(),
        runtime_dir,
    }
}

fn native_tools_available() -> bool {
    fs::metadata("/usr/bin/virsh")
        .is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
        && ["virtqemud", "virtstoraged", "passt"]
            .iter()
            .all(|tool| find_executable(tool).is_some())
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            fs::metadata(candidate)
                .is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
        })
}

fn ensure_flatpak_installed() -> Result<()> {
    for app in [
        "org.virt_manager.virt-manager",
        "org.virt_manager.virt_manager.Extension.Qemu",
    ] {
        let mut command = Command::new("flatpak");
        command.args(["info", app]);
        let output = crate::process::capture(command, Duration::from_secs(20))?;
        if !output.status.success() {
            return Err(Error::Invalid(format!(
                "required Flatpak application is unavailable: {app}"
            )));
        }
    }
    Ok(())
}

fn ensure_private_runtime_dir(path: &Path, owner_uid: u32) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err(Error::Invalid("invalid libvirt runtime directory".into()));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != owner_uid
            {
                return Err(Error::Invalid(
                    "libvirt runtime directory is not a user-owned directory".into(),
                ));
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn validate_user_runtime_dir(path: &Path, owner_uid: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(Error::Invalid(
            "user runtime directory is not a private directory owned by this user".into(),
        ));
    }
    Ok(())
}

fn start_native_daemons(runtime_dir: &Path) -> Result<()> {
    let sockets = [
        runtime_socket(runtime_dir, "virtqemud-sock"),
        runtime_socket(runtime_dir, "virtstoraged-sock"),
    ];
    let qemu_socket =
        fs::metadata(&sockets[0]).is_ok_and(|metadata| metadata.file_type().is_socket());
    let storage_socket =
        fs::metadata(&sockets[1]).is_ok_and(|metadata| metadata.file_type().is_socket());
    match (qemu_socket, storage_socket) {
        (true, true) => return Ok(()),
        (false, false) => (),
        _ => {
            return Err(Error::Invalid(
                "native libvirt runtime has only some expected sockets".into(),
            ));
        }
    }
    for daemon in ["virtqemud", "virtstoraged"] {
        let program = find_executable(daemon)
            .ok_or_else(|| Error::Invalid(format!("required VM tool is unavailable: {daemon}")))?;
        let mut command = Command::new(program);
        command
            .args(["--daemon", "--timeout", "0"])
            .env("XDG_RUNTIME_DIR", runtime_dir);
        let output = crate::process::capture(command, Duration::from_secs(20))?;
        if !output.status.success() {
            return Err(Error::Command {
                operation: daemon.into(),
                code: output.status.code(),
            });
        }
    }
    let mut no_children: [Child; 0] = [];
    wait_for_sockets(&sockets, &mut no_children, runtime_dir)
}

fn start_flatpak_daemons(runtime_dir: &Path, flatpak_runner: &Path) -> Result<()> {
    if !flatpak_runner.is_file() || fs::metadata(flatpak_runner)?.mode() & 0o111 == 0 {
        return Err(Error::Invalid(
            "Flatpak libvirt wrapper is missing or not executable".into(),
        ));
    }
    let log_path = runtime_dir.join("flatpak.log");
    if fs::symlink_metadata(&log_path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(Error::Invalid("Flatpak runtime log is a symlink".into()));
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)?;
    let mut children = Vec::new();
    for daemon in ["virtlogd", "virtstoraged", "virtqemud"] {
        let mut command = Command::new(flatpak_runner);
        command
            .args([daemon, "--timeout", "0"])
            .env("TEST_VM_LIBVIRT_RUNTIME_DIR", runtime_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?))
            .process_group(0);
        match command.spawn() {
            Ok(child) => children.push(child),
            Err(error) => {
                stop_children(&mut children);
                return Err(error.into());
            }
        }
    }
    let sockets = [
        runtime_socket(runtime_dir, "virtqemud-sock"),
        runtime_socket(runtime_dir, "virtstoraged-sock"),
    ];
    wait_for_sockets(&sockets, &mut children, runtime_dir)
}

fn wait_for_sockets(sockets: &[PathBuf], children: &mut [Child], log_dir: &Path) -> Result<()> {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if sockets
            .iter()
            .all(|path| fs::metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket()))
        {
            return Ok(());
        }
        let mut exited_early = false;
        for child in children.iter_mut() {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    exited_early = true;
                }
            }
        }
        if exited_early {
            stop_children(children);
            return Err(Error::Invalid(format!(
                "Flatpak libvirt daemon exited early; inspect {}",
                log_dir.join("flatpak.log").display()
            )));
        }
        if Instant::now() >= deadline {
            stop_children(children);
            return Err(Error::Timeout);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn stop_children(children: &mut [Child]) {
    for child in children.iter_mut() {
        let alive = child.try_wait().ok().flatten().is_none();
        if alive {
            if let Ok(process_group) = i32::try_from(child.id()) {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(process_group),
                    nix::sys::signal::Signal::SIGTERM,
                );
            }
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

fn runtime_socket(runtime_dir: &Path, name: &str) -> PathBuf {
    runtime_dir.join("libvirt").join(name)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
