use std::process::Command;
use std::sync::LazyLock;
use thiserror::Error;
use tracing::{debug, info, warn};
use users::get_group_by_name;
use zbus::proxy;

static SYSTEMD_UNIT_SUFFIXES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        ".service",
        ".socket",
        ".device",
        ".mount",
        ".automount",
        ".swap",
        ".target",
        ".path",
        ".timer",
        ".slice",
        ".scope",
    ]
});

fn ensure_systemd_suffix(name: &str) -> String {
    if SYSTEMD_UNIT_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        name.to_string()
    } else {
        format!("{name}.service")
    }
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait SystemdManager {
    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    fn reload_unit(&self, name: &str, mode: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[derive(Error, Debug)]
pub enum SystemError {
    #[error("Group check error: {0}")]
    GroupCheck(String),
    #[error("Command failed: {0}")]
    Command(String),
    #[error("DBus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub trait SystemResource {
    fn ensure_group(&self, name: &str, gid: Option<u32>) -> Result<bool, SystemError>;
    fn ensure_user(
        &self,
        name: &str,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<bool, SystemError>;
    fn ensure_podman_secret(&self, name: &str, payload: &str) -> Result<bool, SystemError>;
    fn service_start(&self, name: &str) -> Result<(), SystemError>;
    fn service_stop(&self, name: &str) -> Result<(), SystemError>;
    fn service_restart(&self, name: &str) -> Result<(), SystemError>;
    fn service_reload(&self, name: &str) -> Result<(), SystemError>;
}

pub struct LinuxSystemResource {
    conn: Option<zbus::blocking::Connection>,
}

impl LinuxSystemResource {
    pub fn new() -> Self {
        let conn = match zbus::blocking::Connection::system() {
            Ok(c) => Some(c),
            Err(e) => {
                warn!("Failed to connect to system DBus, will fallback to CLI: {e}");
                None
            }
        };
        Self { conn }
    }

    fn run_systemctl(&self, action: &str, name: &str) -> Result<(), SystemError> {
        let name_with_suffix = ensure_systemd_suffix(name);

        if let Some(conn) = &self.conn {
            info!("Running systemctl {action} {name_with_suffix} via DBus");
            let proxy = SystemdManagerProxyBlocking::new(conn)?;
            let res = match action {
                "start" => proxy.start_unit(&name_with_suffix, "replace"),
                "stop" => proxy.stop_unit(&name_with_suffix, "replace"),
                "restart" => proxy.restart_unit(&name_with_suffix, "replace"),
                "reload" => proxy.reload_unit(&name_with_suffix, "replace"),
                _ => {
                    return Err(SystemError::Command(format!("Unsupported action: {action}")));
                }
            };

            match res {
                Ok(_) => return Ok(()),
                Err(e) => {
                    warn!("DBus call failed, falling back to CLI: {e}");
                }
            }
        }

        info!("Running systemctl {action} {name_with_suffix} via CLI");
        let output = Command::new("systemctl")
            .arg(action)
            .arg(&name_with_suffix)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SystemError::Command(format!(
                "systemctl {action} {name_with_suffix} failed: {stderr}"
            )));
        }
        Ok(())
    }
}

impl Default for LinuxSystemResource {
    fn default() -> Self {
        Self::new()
    }
}

const EXIT_CODE_GROUP_EXISTS: i32 = 9;
const EXIT_CODE_USER_EXISTS: i32 = 9;

impl SystemResource for LinuxSystemResource {
    fn ensure_group(&self, name: &str, gid: Option<u32>) -> Result<bool, SystemError> {
        if get_group_by_name(name).is_some() {
            debug!("Group {name} already exists");
            return Ok(false);
        }

        info!("Creating group {name}");
        let mut cmd = Command::new("groupadd");
        if let Some(g) = gid {
            cmd.arg("-g").arg(g.to_string());
        }
        cmd.arg(name);
        let output = cmd.output()?;

        if !output.status.success() {
            if output.status.code() == Some(EXIT_CODE_GROUP_EXISTS) {
                debug!("Group {name} was created by another process");
                return Ok(false);
            }

            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SystemError::Command(format!("groupadd failed: {stderr}")));
        }

        info!("Created group {name}");
        Ok(true)
    }

    fn ensure_user(
        &self,
        name: &str,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<bool, SystemError> {
        if users::get_user_by_name(name).is_some() {
            debug!("User {name} already exists");
            return Ok(false);
        }

        if let Some(gid_val) = gid {
            self.ensure_group(name, Some(gid_val))?;
        }

        info!("Creating user {name}");
        let mut cmd = Command::new("useradd");
        if let Some(u) = uid {
            cmd.arg("-u").arg(u.to_string());
        }
        if let Some(g) = gid {
            cmd.arg("-g").arg(g.to_string());
        }
        cmd.arg(name);

        let output = cmd.output()?;

        if !output.status.success() {
            if output.status.code() == Some(EXIT_CODE_USER_EXISTS) {
                debug!("User {name} was created by another process");
                return Ok(false);
            }

            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SystemError::Command(format!("useradd failed: {stderr}")));
        }

        info!("Created user {name}");
        Ok(true)
    }

    fn ensure_podman_secret(&self, name: &str, payload: &str) -> Result<bool, SystemError> {
        let inspect_output = Command::new("podman")
            .args(["secret", "inspect", name])
            .output()?;

        if inspect_output.status.success() {
            debug!("Podman secret {name} already exists");
            return Ok(false);
        }

        info!("Creating podman secret {name}");
        let mut child = Command::new("podman")
            .args(["secret", "create", name, "-"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write as _;
            stdin.write_all(payload.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(SystemError::Command(format!(
                "podman secret create {name} failed"
            )));
        }

        Ok(true)
    }

    fn service_start(&self, name: &str) -> Result<(), SystemError> {
        self.run_systemctl("start", name)
    }

    fn service_stop(&self, name: &str) -> Result<(), SystemError> {
        self.run_systemctl("stop", name)
    }

    fn service_restart(&self, name: &str) -> Result<(), SystemError> {
        self.run_systemctl("restart", name)
    }

    fn service_reload(&self, name: &str) -> Result<(), SystemError> {
        self.run_systemctl("reload", name)
    }
}

#[cfg(test)]
#[path = "system/tests.rs"]
mod tests;
