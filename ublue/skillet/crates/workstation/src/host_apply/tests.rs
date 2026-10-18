use super::*;
use std::{
    cell::RefCell,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{ExitStatus, Output},
};

#[derive(Default)]
struct Recorder {
    events: Vec<Step>,
    fail: Option<Step>,
}
impl Actions for Recorder {
    type Error = &'static str;
    fn stage(&mut self, service: &'static str) -> Result<(), Self::Error> {
        self.record(Step::Stage(service))
    }
    fn activate(&mut self, unit: &'static str) -> Result<(), Self::Error> {
        self.record(Step::Activate(unit))
    }
}
impl Recorder {
    fn record(&mut self, step: Step) -> Result<(), &'static str> {
        self.events.push(step);
        if self.fail == Some(step) {
            return Err("injected failure");
        }
        Ok(())
    }
}

#[test]
fn related_credentials_stage_before_full_apply_and_monitoring_is_last() {
    for host in ["clamps", "joey"] {
        let profile = skillet_hosts::profile_for_name(host).unwrap();
        let mut actions = Recorder::default();
        run(&profile, &mut actions).unwrap();
        assert_eq!(actions.events, steps(&profile));
        let baseline = actions
            .events
            .iter()
            .position(|step| *step == Step::Activate("skillet-full-apply.service"))
            .unwrap();
        assert!(!actions.events.contains(&Step::Stage("pihole")));
        for service in ["tailscale", "syncthing"] {
            assert!(
                actions
                    .events
                    .iter()
                    .position(|step| *step == Step::Stage(service))
                    .unwrap()
                    < baseline
            );
        }
        assert!(
            actions
                .events
                .iter()
                .position(|step| *step == Step::Stage("caddy"))
                .unwrap()
                > baseline
        );
        assert_eq!(
            actions.events.last(),
            Some(&Step::Activate("skillet-datadog-apply.service"))
        );
    }
}

#[test]
fn optional_phases_follow_only_canonical_host_capabilities() {
    let mut profile = skillet_hosts::profile_for_name("clamps").unwrap();
    profile.services.retain(|service| {
        !matches!(
            service.config,
            skillet_hosts::ServiceConfig::Ddns { .. }
                | skillet_hosts::ServiceConfig::Datadog { .. }
                | skillet_hosts::ServiceConfig::Smtp
                | skillet_hosts::ServiceConfig::Network(_)
                | skillet_hosts::ServiceConfig::HomeAssistant(_)
        )
    });
    for service in &mut profile.services {
        service.ui = None;
    }
    assert_eq!(
        steps(&profile),
        [
            Step::Activate("skillet-apply.service"),
            Step::Stage("pihole"),
            Step::Stage("tailscale"),
            Step::Stage("syncthing"),
            Step::Activate("skillet-syncthing-initialize.service"),
            Step::Activate("skillet-full-apply.service"),
            Step::Stage("pihole-dns"),
            Step::Activate("skillet-pihole-dns-apply.service")
        ]
    );
}

#[test]
fn every_failure_stops_later_steps_and_can_retry() {
    let profile = skillet_hosts::profile_for_name("clamps").unwrap();
    for failure in steps(&profile) {
        let mut actions = Recorder {
            fail: Some(failure),
            ..Recorder::default()
        };
        assert!(run(&profile, &mut actions).is_err());
        let end = steps(&profile)
            .iter()
            .position(|step| *step == failure)
            .unwrap();
        assert_eq!(actions.events, steps(&profile)[..=end]);
        actions.fail = None;
        actions.events.clear();
        run(&profile, &mut actions).unwrap();
        assert_eq!(actions.events, steps(&profile));
    }
}

struct Guest {
    status: String,
    journal_after: String,
    calls: RefCell<Vec<Vec<String>>>,
}
impl GuestTransport for Guest {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        assert!(input.is_none());
        self.calls
            .borrow_mut()
            .push(command.arguments.iter().map(|arg| (*arg).into()).collect());
        let output = if command.arguments.contains(&"--show-cursor") {
            "-- cursor: before-start\n".into()
        } else if command.arguments.contains(&"--after-cursor") {
            self.journal_after.clone()
        } else if command.arguments.contains(&"show") {
            self.status.clone()
        } else {
            String::new()
        };
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: output.into_bytes(),
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        panic!("unexpected upload")
    }
}

fn successful_journal_entry(unit: &str) -> String {
    serde_json::json!({
        "MESSAGE_ID": "39f53479d3a045ac8e11786248231fbf",
        "UNIT": unit,
        "JOB_TYPE": "start",
        "JOB_RESULT": "done",
        "INVOCATION_ID": "test-invocation",
    })
    .to_string()
}

#[test]
fn successful_runtime_state_remains_a_valid_execution_proof() {
    let guest = Guest {
        status: "LoadState=loaded\nConditionResult=yes\nResult=success\nExecMainStatus=0\nExecMainStartTimestampMonotonic=123\n".into(),
        journal_after: String::new(),
        calls: RefCell::default(),
    };
    activate(&guest, "skillet-full-apply.service").unwrap();
    assert!(!guest.calls.borrow().iter().any(|arguments| {
        arguments
            .iter()
            .any(|argument| argument == "--after-cursor")
    }));
}

#[test]
fn completed_inactive_oneshot_is_verified_from_the_journal() {
    let guest = Guest {
        status: "LoadState=loaded\nConditionResult=no\nResult=success\nExecMainStatus=0\nExecMainStartTimestampMonotonic=0\n".into(),
        journal_after: successful_journal_entry("skillet-full-apply.service"),
        calls: RefCell::default(),
    };
    activate(&guest, "skillet-full-apply.service").unwrap();
    assert!(guest.calls.borrow().iter().any(|arguments| {
        arguments
            .windows(2)
            .any(|pair| pair == ["--after-cursor", "before-start"])
    }));
    assert!(!guest
        .calls
        .borrow()
        .iter()
        .flatten()
        .any(|argument| argument == "--wait"));
}

#[test]
fn skipped_or_failed_oneshot_is_not_success() {
    let skipped = serde_json::json!({
        "UNIT": "skillet-full-apply.service",
        "JOB_TYPE": "start",
        "JOB_RESULT": "done",
    })
    .to_string();
    let failed = serde_json::json!({
        "MESSAGE_ID": "39f53479d3a045ac8e11786248231fbf",
        "UNIT": "skillet-full-apply.service",
        "JOB_TYPE": "start",
        "JOB_RESULT": "failed",
        "INVOCATION_ID": "test-invocation",
    })
    .to_string();
    for journal_after in [String::new(), skipped, failed] {
        let guest = Guest {
            status: "LoadState=loaded\nConditionResult=no\nResult=success\nExecMainStatus=0\nExecMainStartTimestampMonotonic=0\n".into(),
            journal_after,
            calls: RefCell::default(),
        };
        assert!(activate(&guest, "skillet-full-apply.service").is_err());
    }
}
