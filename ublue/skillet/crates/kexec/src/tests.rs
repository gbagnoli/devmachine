use super::*;

#[test]
fn activation_selection_uses_latest_pending_and_rejects_active_transactions() {
    let mut status = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "new", "staged": true, "booted": false},
        {"checksum": "old", "staged": false, "booted": true}
    ]});
    assert_eq!(activation_target(&status).unwrap()["checksum"], "new");
    status["deployments"][0]["checksum"] = serde_json::json!("replacement");
    assert_eq!(
        activation_target(&status).unwrap()["checksum"],
        "replacement"
    );
    status["transaction"] = serde_json::json!(["update", "initiator", "address"]);
    assert!(activation_target(&status).is_err());
    assert!(activation_target(&serde_json::json!({"deployments": []})).is_err());
    let multiple_staged = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "newer", "staged": true, "booted": false},
        {"checksum": "new", "staged": true, "booted": false},
        {"checksum": "old", "staged": false, "booted": true}
    ]});
    assert!(activation_target(&multiple_staged).is_err());
    let no_update = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "current", "staged": false, "booted": true}
    ]});
    assert!(activation_target(&no_update).is_err());
}

#[test]
fn activation_hook_recognizes_staged_and_previously_finalized_targets() {
    let staged = serde_json::json!({"deployments": [
        {"checksum": "current", "staged": false, "booted": true}
    ]});
    assert!(!has_activation_target(&staged).unwrap());

    let update = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "next", "staged": true, "booted": false, "timestamp": 20},
        {"checksum": "current", "staged": false, "booted": true, "timestamp": 10}
    ]});
    assert!(has_activation_target(&update).unwrap());
    assert_eq!(activation_target(&update).unwrap()["checksum"], "next");

    let finalized = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "next", "staged": false, "booted": false, "timestamp": 20},
        {"checksum": "current", "staged": false, "booted": true, "timestamp": 10}
    ]});
    assert!(has_activation_target(&finalized).unwrap());
    assert_eq!(activation_target(&finalized).unwrap()["checksum"], "next");

    let old_rollback = serde_json::json!({"transaction": null, "deployments": [
        {"checksum": "old", "staged": false, "booted": false, "timestamp": 5},
        {"checksum": "current", "staged": false, "booted": true, "timestamp": 10}
    ]});
    assert!(!has_activation_target(&old_rollback).unwrap());
    assert!(has_activation_target(&serde_json::json!({})).is_err());
}

#[test]
fn first_sunday_normal_reboot_follows_staging_even_without_an_update() {
    use time::{Date, Month, Time};

    let at = |year, month, day| {
        Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_time(Time::from_hms(4, 30, 0).unwrap())
            .assume_utc()
    };
    let no_update = serde_json::json!({"deployments": [
        {"checksum": "current", "staged": false, "booted": true}
    ]});
    let update = serde_json::json!({"deployments": [
        {"checksum": "next", "staged": true, "booted": false},
        {"checksum": "current", "staged": false, "booted": true}
    ]});
    let first_sunday = at(2026, Month::November, 1);
    let ordinary_sunday = at(2026, Month::November, 8);
    let monday = at(2026, Month::November, 2);

    assert_eq!(
        activation_action(&no_update, first_sunday).unwrap(),
        ActivationAction::NormalReboot
    );
    assert_eq!(
        activation_action(&update, first_sunday).unwrap(),
        ActivationAction::NormalReboot
    );
    assert_eq!(
        activation_action(&no_update, ordinary_sunday).unwrap(),
        ActivationAction::Noop
    );
    assert_eq!(
        activation_action(&update, monday).unwrap(),
        ActivationAction::Kexec
    );
}

#[derive(Default)]
struct FakeActivation {
    armed: bool,
    load_error: bool,
    cleanup_error: bool,
    unloads: usize,
}

impl attempt::Activation for FakeActivation {
    type Error = &'static str;
    fn is_armed(&mut self) -> Result<bool, Self::Error> {
        Ok(self.armed)
    }
    fn prepare(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn load(&mut self) -> Result<(), Self::Error> {
        self.armed = true;
        if self.load_error {
            Err("load failed after arming")
        } else {
            Ok(())
        }
    }
    fn handoff(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn unload(&mut self) -> Result<(), Self::Error> {
        self.unloads += 1;
        if self.cleanup_error {
            Err("unload failed")
        } else {
            self.armed = false;
            Ok(())
        }
    }
}

#[test]
fn failed_load_and_returning_handoff_both_clear_owned_kernel() {
    for load_error in [false, true] {
        let mut activation = FakeActivation {
            load_error,
            ..Default::default()
        };
        assert!(attempt::attempt(&mut activation).is_err());
        assert_eq!(activation.unloads, 1);
        assert!(!activation.armed);
    }
}

#[test]
fn foreign_kernel_is_never_unloaded_and_cleanup_failure_is_visible() {
    let mut activation = FakeActivation {
        armed: true,
        ..Default::default()
    };
    assert_eq!(
        attempt::attempt(&mut activation),
        Err(attempt::AttemptError::ForeignKernel)
    );
    assert_eq!(activation.unloads, 0);
    let mut activation = FakeActivation {
        cleanup_error: true,
        ..Default::default()
    };
    assert!(matches!(
        attempt::attempt(&mut activation),
        Err(attempt::AttemptError::Cleanup { .. })
    ));
    assert!(activation.armed);
}

#[test]
fn reads_kernel_initrd_and_exact_deployment_selector_without_shell_interpretation() {
    let entry = parse_entry("title Fedora\nlinux /ostree/x/vmlinuz\ninitrd /ostree/x/initramfs.img\noptions root=UUID=example ostree=/ostree/boot.1/fedora/x/0 console=ttyS0\n", Path::new("/boot")).unwrap();
    assert_eq!(entry.kernel, Path::new("/boot/ostree/x/vmlinuz"));
    assert_eq!(entry.initrd, Path::new("/boot/ostree/x/initramfs.img"));
    assert_eq!(
        entry.deployment_selector,
        Path::new("/ostree/boot.1/fedora/x/0")
    );
    assert!(entry.options.contains("console=ttyS0"));
}

#[test]
fn rejects_duplicate_paths_missing_selectors_and_parent_traversal() {
    for input in [
        "linux /a\nlinux /b\ninitrd /c\noptions ostree=/ostree/boot.1/x\n",
        "linux /a\ninitrd /b\ninitrd /c\noptions ostree=/ostree/boot.1/x\n",
        "linux /a\ninitrd /b\noptions quiet\n",
        "linux /a\ninitrd /b\noptions ostree=/ostree/boot.1/x ostree=/ostree/boot.1/y\n",
        "linux /../etc/passwd\ninitrd /b\noptions ostree=/ostree/boot.1/x\n",
        "linux /a\ninitrd /b\noptions ostree=/ostree/boot.1/../x\n",
        "linux relative\ninitrd /b\noptions ostree=/ostree/boot.1/x\n",
    ] {
        assert!(parse_entry(input, Path::new("/boot")).is_err(), "{input}");
    }
}

#[test]
fn requires_one_exact_resolved_deployment_including_serial() {
    let one = "linux /k1\ninitrd /i1\noptions ostree=/ostree/boot.1/a/0\n";
    let two = "linux /k2\ninitrd /i2\noptions ostree=/ostree/boot.1/a/1\n";
    let resolve = |path: &Path| {
        let mut parts = path.components();
        let checksum = parts.nth(3)?.as_os_str().to_str()?;
        let serial = parts.next()?.as_os_str().to_str()?;
        Some(PathBuf::from(format!("/deploy/{checksum}.{serial}")))
    };
    let selected = select_entry(
        [one, two],
        Path::new("/boot"),
        Path::new("/deploy/a.1"),
        resolve,
    )
    .unwrap();
    assert_eq!(selected.kernel, Path::new("/boot/k2"));
    assert!(select_entry(
        [one, one],
        Path::new("/boot"),
        Path::new("/deploy/a.0"),
        resolve
    )
    .is_err());
    assert!(select_entry([one], Path::new("/boot"), Path::new("/deploy/a.1"), resolve).is_err());
    assert!(
        select_entry([one], Path::new("/boot"), Path::new("/deploy/a.0"), |_| {
            None
        })
        .is_err()
    );
}
