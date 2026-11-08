use super::*;
use std::cell::{Cell, RefCell};

const PORTS: &[FirewallPort] = &[
    FirewallPort {
        port: 8080,
        protocol: FirewallProtocol::Tcp,
    },
    FirewallPort {
        port: 3478,
        protocol: FirewallProtocol::Udp,
    },
];

#[derive(Default)]
struct Fake {
    rules: RefCell<BTreeSet<(bool, String, String)>>,
    added: RefCell<Vec<bool>>,
    fail_runtime_once: Cell<bool>,
}
impl Firewalld for Fake {
    fn zone(&self, _: &str) -> Result<String, SystemError> {
        Ok("home-test".into())
    }
    fn contains(&self, zone: &str, rule: &str, permanent: bool) -> Result<bool, SystemError> {
        Ok(self
            .rules
            .borrow()
            .contains(&(permanent, zone.into(), rule.into())))
    }
    fn add(&self, zone: &str, rule: &str, permanent: bool) -> Result<(), SystemError> {
        if !permanent && self.fail_runtime_once.replace(false) {
            return Err(SystemError::Firewall("injected runtime failure".into()));
        }
        self.added.borrow_mut().push(permanent);
        assert!(self
            .rules
            .borrow_mut()
            .insert((permanent, zone.into(), rule.into())));
        Ok(())
    }
}
fn lan() -> Lan {
    Lan {
        interface: "testnic".into(),
        subnet: "192.168.20.0/24".into(),
    }
}

#[test]
fn sources_are_private_subnets_and_masks_cannot_include_public_addresses() {
    for (address, mask, expected) in [
        ("192.168.20.5", "255.255.255.0", Some("192.168.20.0/24")),
        ("10.2.3.4", "255.0.0.0", Some("10.0.0.0/8")),
        ("172.19.4.2", "255.240.0.0", Some("172.16.0.0/12")),
        ("192.0.2.5", "255.255.255.0", None),
        ("100.80.1.5", "255.255.255.0", None),
        ("127.0.0.1", "255.0.0.0", None),
        ("172.19.4.2", "255.0.0.0", None),
        ("192.168.20.5", "255.0.255.0", None),
        ("192.168.20.5", "0.0.0.0", None),
        ("192.168.20.5", "255.255.255.255", None),
    ] {
        assert_eq!(
            private_subnet(address.parse().unwrap(), mask.parse().unwrap()).as_deref(),
            expected
        );
    }
}
#[test]
fn rules_are_scoped_persisted_first_deduplicated_and_idempotent() {
    let backend = Fake::default();
    let unrelated = (true, "home-test".into(), "unrelated operator rule".into());
    backend.rules.borrow_mut().insert(unrelated.clone());
    assert!(converge(&backend, &[lan(), lan()], PORTS, MissingLanPolicy::Reject).unwrap());
    assert_eq!(*backend.added.borrow(), [true, false, true, false]);
    assert!(!converge(&backend, &[lan()], PORTS, MissingLanPolicy::Reject).unwrap());
    assert!(backend.rules.borrow().contains(&unrelated));
    for entry in backend
        .rules
        .borrow()
        .iter()
        .filter(|entry| *entry != &unrelated)
    {
        assert!(entry.2.contains("source address=\"192.168.20.0/24\""));
        assert!(!entry.2.contains("8443"));
    }
}
#[test]
fn interruption_and_either_kind_of_drift_are_repaired_without_duplicate_adds() {
    let backend = Fake::default();
    backend.fail_runtime_once.set(true);
    assert!(converge(&backend, &[lan()], &PORTS[..1], MissingLanPolicy::Reject).is_err());
    assert_eq!(*backend.added.borrow(), [true]);
    assert!(converge(&backend, &[lan()], &PORTS[..1], MissingLanPolicy::Reject).unwrap());
    for permanent in [false, true] {
        backend
            .rules
            .borrow_mut()
            .retain(|entry| entry.0 != permanent);
        assert!(converge(&backend, &[lan()], &PORTS[..1], MissingLanPolicy::Reject).unwrap());
        assert!(!converge(&backend, &[lan()], &PORTS[..1], MissingLanPolicy::Reject).unwrap());
    }
}
#[test]
fn missing_scope_and_invalid_ports_fail_closed_before_any_mutation() {
    let backend = Fake::default();
    assert!(converge(&backend, &[], PORTS, MissingLanPolicy::Reject).is_err());
    assert!(!converge(&backend, &[], PORTS, MissingLanPolicy::Skip).unwrap());
    assert!(converge(
        &backend,
        &[lan()],
        &[FirewallPort {
            port: 0,
            protocol: FirewallProtocol::Tcp
        }],
        MissingLanPolicy::Reject
    )
    .is_err());
    assert!(backend.rules.borrow().is_empty());
    assert!(!converge(&backend, &[], &[], MissingLanPolicy::Reject).unwrap());
}
#[test]
fn recorder_reports_changes_without_recording_lan_addresses() {
    use crate::{recorder::Recorder, resource_op::EffectResult, test_utils::MockSystem};
    let recorder = Recorder::new(MockSystem::new());
    assert!(recorder
        .ensure_private_ipv4_lan_ports(PORTS, MissingLanPolicy::Reject)
        .unwrap());
    assert!(!recorder
        .ensure_private_ipv4_lan_ports(PORTS, MissingLanPolicy::Reject)
        .unwrap());
    let operations = recorder.get_ops();
    assert_eq!(operations[0].result, EffectResult::Changed(true));
    assert_eq!(operations[1].result, EffectResult::Changed(false));
}

#[test]
fn retained_recording_requires_a_lan_unless_skipping_is_explicit() {
    let operation: crate::resource_op::ResourceOp =
        yaml_serde::from_str("!EnsurePrivateIpv4LanPorts {ports: [{port: 2022, protocol: tcp}]}")
            .unwrap();
    assert!(matches!(
        operation,
        crate::resource_op::ResourceOp::EnsurePrivateIpv4LanPorts {
            missing_lan: MissingLanPolicy::Reject,
            ..
        }
    ));
    assert!(converge(
        &Fake::default(),
        &[],
        &[FirewallPort {
            port: 0,
            protocol: FirewallProtocol::Tcp
        }],
        MissingLanPolicy::Skip
    )
    .is_err());
}

fn remove(
    backend: &DbusFirewall,
    zone: &str,
    rule: &str,
    permanent: bool,
) -> Result<(), SystemError> {
    if backend.contains(zone, rule, permanent)? {
        if permanent {
            backend
                .permanent(zone)?
                .call_method("removeRichRule", &(rule,))?;
        } else {
            backend
                .runtime()?
                .call_method("removeRichRule", &(zone, rule))?;
        }
    }
    Ok(())
}
struct Cleanup<'a> {
    backend: &'a DbusFirewall,
    rules: Vec<(String, String)>,
}
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for (zone, rule) in &self.rules {
            for permanent in [false, true] {
                let _ = remove(self.backend, zone, rule, permanent);
            }
        }
    }
}
#[test]
#[ignore = "requires root, firewalld and an owned disposable VM"]
fn real_firewalld_repairs_runtime_and_permanent_lan_grants() {
    assert!(fs::read_to_string("/etc/hostname")
        .unwrap()
        .trim()
        .contains("-test-"));
    assert!(std::process::Command::new("systemd-detect-virt")
        .arg("--vm")
        .status()
        .unwrap()
        .success());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let ports = [FirewallPort {
        port: listener.local_addr().unwrap().port(),
        protocol: FirewallProtocol::Tcp,
    }];
    let lans = observe_lans().unwrap();
    assert_ne!(lans, []);
    let backend = DbusFirewall(Connection::system().unwrap());
    let rules = lans
        .iter()
        .map(|lan| (backend.zone(&lan.interface).unwrap(), rule(lan, ports[0])))
        .collect::<BTreeSet<_>>();
    for (zone, rule) in &rules {
        assert!(!backend.contains(zone, rule, false).unwrap());
        assert!(!backend.contains(zone, rule, true).unwrap());
    }
    let cleanup = Cleanup {
        backend: &backend,
        rules: rules.into_iter().collect(),
    };
    assert!(converge(&backend, &lans, &ports, MissingLanPolicy::Reject).unwrap());
    assert!(!converge(&backend, &lans, &ports, MissingLanPolicy::Reject).unwrap());
    for permanent in [false, true] {
        for (zone, rule) in &cleanup.rules {
            remove(&backend, zone, rule, permanent).unwrap();
        }
        assert!(converge(&backend, &lans, &ports, MissingLanPolicy::Reject).unwrap());
        assert!(!converge(&backend, &lans, &ports, MissingLanPolicy::Reject).unwrap());
    }
    for (zone, rule) in &cleanup.rules {
        assert!(backend.contains(zone, rule, false).unwrap());
        assert!(backend.contains(zone, rule, true).unwrap());
    }
    drop(cleanup);
    println!("Verified live firewalld convergence, no-op repeat apply and repair of runtime/permanent drift; temporary test rules removed");
}
