use super::{application_snapshot, application_snapshot_with_probe_timeout, require_data_mount};
use skillet_hosts::{profile_for_name, HostAcceptancePlan};
use skillet_vm::transport::{GuestCommand, GuestTransport};
use std::{
    os::unix::process::ExitStatusExt as _,
    path::Path,
    process::{ExitStatus, Output},
};

struct AcceptanceTransport {
    active: bool,
    network: String,
    missing_listener: Option<u16>,
    owner: String,
    container_id: String,
}

impl GuestTransport for AcceptanceTransport {
    fn execute(&self, request: &GuestCommand<'_>, _: Option<&[u8]>) -> skillet_vm::Result<Output> {
        let arguments = request.arguments;
        let output = match (request.program, arguments) {
            ("systemctl", ["is-active", _]) if self.active => b"active\n".to_vec(),
            ("systemctl", ["is-active", _]) => b"inactive\n".to_vec(),
            ("sudo", ["-n", "podman", "inspect", "--format", _, _]) => {
                format!(
                    "true|bridge|{}|{{\"{}\":{{}}}}|/var/lib/data/syncthing;/var/lib/data/caddy/data;\n",
                    self.container_id, self.network
                )
                .into_bytes()
            }
            ("id", ["-u", "giacomo"]) => b"1042\n".to_vec(),
            ("getent", ["group", "giacomo"]) => b"giacomo:x:2047:\n".to_vec(),
            ("stat", ["-c", "%u:%g", _]) => format!("{}\n", self.owner).into_bytes(),
            ("sudo", ["-n", "ss", "-H", _, "sport", "=", port])
                if self.missing_listener != port.trim_start_matches(':').parse().ok() =>
            {
                b"LISTEN 0 128 0.0.0.0:port *:*\n".to_vec()
            }
            ("sha256sum", [_]) => b"abc123 /etc/containers/systemd/test.container\n".to_vec(),
            _ => Vec::new(),
        };
        let success = !matches!((request.program, arguments), ("systemctl", ["is-active", _]) if !self.active);
        Ok(Output {
            status: ExitStatus::from_raw(if success { 0 } else { 1 << 8 }),
            stdout: output,
            stderr: Vec::new(),
        })
    }

    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        Ok(())
    }
}

fn transport() -> AcceptanceTransport {
    AcceptanceTransport {
        active: true,
        network: "joey".into(),
        missing_listener: None,
        owner: "1042:2047".into(),
        container_id: "container-id".into(),
    }
}

// These observations exercise the Syncthing/Caddy capability subset only.
// Host selection itself is checked against the complete canonical declaration.
fn syncthing_ui_plan() -> HostAcceptancePlan {
    let mut plan = profile_for_name("joey").unwrap().acceptance_plan();
    plan.services
        .retain(|service| matches!(service.unit.as_str(), "syncthing.service" | "caddy.service"));
    plan
}

#[test]
fn application_snapshot_accepts_profile_declared_runtime() {
    let plan = syncthing_ui_plan();
    let observed = application_snapshot(&transport(), &|| Ok(()), &plan).unwrap();
    assert!(observed
        .iter()
        .any(|entry| entry.starts_with("container:syncthing:")));
    assert!(observed
        .iter()
        .any(|entry| entry.starts_with("container:caddy:")));
}

#[test]
fn application_snapshot_rejects_stopped_units() {
    let mut fake = transport();
    fake.active = false;
    let plan = syncthing_ui_plan();
    assert!(application_snapshot_with_probe_timeout(
        &fake,
        &|| Ok(()),
        &plan,
        std::time::Duration::ZERO
    )
    .unwrap_err()
    .to_string()
    .contains("not become active"));
}

#[test]
fn application_snapshot_rejects_wrong_network_and_missing_listener() {
    let plan = syncthing_ui_plan();
    let mut wrong_network = transport();
    wrong_network.network = "unrelated".into();
    assert!(application_snapshot(&wrong_network, &|| Ok(()), &plan)
        .unwrap_err()
        .to_string()
        .contains("network attachment"));

    let mut no_listener = transport();
    no_listener.missing_listener = Some(22000);
    assert!(application_snapshot_with_probe_timeout(
        &no_listener,
        &|| Ok(()),
        &plan,
        std::time::Duration::ZERO
    )
    .unwrap_err()
    .to_string()
    .contains("listener"));
}

#[test]
fn application_snapshot_rejects_wrong_data_ownership_and_changed_identity() {
    let plan = syncthing_ui_plan();
    let mut wrong_owner = transport();
    wrong_owner.owner = "1000:1000".into();
    assert!(application_snapshot(&wrong_owner, &|| Ok(()), &plan)
        .unwrap_err()
        .to_string()
        .contains("ownership"));

    let before = application_snapshot(&transport(), &|| Ok(()), &plan).unwrap();
    let mut changed = transport();
    changed.container_id = "recreated-container".into();
    let after = application_snapshot(&changed, &|| Ok(()), &plan).unwrap();
    assert_ne!(before, after);
}

#[test]
fn data_mount_and_persistence_hash_checks_reject_lost_state() {
    assert!(require_data_mount("btrfs /data").is_ok());
    assert!(require_data_mount("xfs /").is_err());

    let transport = transport();
    assert!(
        super::verify_guest_hash(&transport, "/var/lib/data/.marker", "expected-hash").is_err()
    );
}

struct FilesTransport {
    sibling_exit: i32,
    probes: std::sync::Mutex<Vec<String>>,
}

impl GuestTransport for FilesTransport {
    fn execute(&self, request: &GuestCommand<'_>, _: Option<&[u8]>) -> skillet_vm::Result<Output> {
        let (stdout, code) = match (request.program, request.arguments) {
            ("sudo", ["-n", "podman", "inspect", "--format", _, "mikochi"]) => (
                br#"{"fixture-files":{"IPAddress":"172.28.0.2","GlobalIPv6Address":"fd00:1234::2"}}"#.to_vec(),
                0,
            ),
            ("curl", arguments) => {
                self.probes.lock().unwrap().push(arguments.last().unwrap().to_string());
                (br#"{"status":"ok"}"#.to_vec(), 0)
            }
            ("sudo", ["-n", "podman", "exec", "syncthing", "curl", arguments @ ..]) => {
                self.probes.lock().unwrap().push(arguments.last().unwrap().to_string());
                (Vec::new(), self.sibling_exit)
            }
            _ => return Err(skillet_vm::Error::Invalid("unsupported acceptance fake request".into())),
        };
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout,
            stderr: Vec::new(),
        })
    }

    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        Err(skillet_vm::Error::Invalid(
            "unsupported acceptance fake upload".into(),
        ))
    }
}

#[test]
fn file_ui_health_proves_readiness_and_sibling_denial_in_both_ip_families() {
    for sibling_exit in [7, 28] {
        let guest = FilesTransport {
            sibling_exit,
            probes: std::sync::Mutex::default(),
        };
        super::check_mikochi_health(&guest).unwrap();
        let probes = guest.probes.lock().unwrap();
        assert_eq!(probes.len(), 4);
        assert_eq!(probes[0], probes[1]);
        assert_eq!(probes[2], probes[3]);
        assert!(probes[0].contains("172.28.0.2:8080"));
        assert!(probes[2].contains("[fd00:1234::2]:8080"));
    }
}

#[test]
fn file_ui_health_rejects_accessible_backends_and_broken_probe_commands() {
    for sibling_exit in [0, 1, 3, 127] {
        let guest = FilesTransport {
            sibling_exit,
            probes: std::sync::Mutex::default(),
        };
        assert!(super::check_mikochi_health(&guest).is_err());
    }
}
