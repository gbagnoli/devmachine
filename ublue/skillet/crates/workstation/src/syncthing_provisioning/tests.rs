use super::*;
use crate::provisioning_policy::Environment;
#[test]
fn authoritative_zone_and_runtime_scope_are_explicit() {
    let prod = ProvisioningPolicy::new(Environment::Production);
    let test = ProvisioningPolicy::new(Environment::Test);
    let input = settings("clamps", "clamps", prod, "example.invalid").unwrap();
    assert_eq!(input.device_name, "clamps.example.invalid");
    let input = settings("joey", "joey-test-seed", test, "example.invalid").unwrap();
    assert_eq!(input.device_name, "joey-test-seed.example.invalid");
    assert!(settings("joey", "joey-test-seed", prod, "example.invalid").is_err());
    assert!(settings("unknown", "unknown", test, "example.invalid").is_err());
    assert!(settings("clamps", "clamps", prod, "bad/zone").is_err());
}

#[test]
fn donbot_delivery_scopes_each_bootstrap_and_defers_both_consumers() {
    use std::{
        cell::RefCell,
        os::unix::process::ExitStatusExt,
        path::Path,
        process::{ExitStatus, Output},
    };
    #[derive(Default)]
    struct Guest(RefCell<Vec<(Vec<String>, Settings)>>);
    impl GuestTransport for Guest {
        fn execute(
            &self,
            command: &skillet_vm::transport::GuestCommand<'_>,
            input: Option<&[u8]>,
        ) -> skillet_vm::Result<Output> {
            self.0.borrow_mut().push((
                command.arguments.iter().map(|v| (*v).into()).collect(),
                Settings::parse(input.unwrap()).unwrap(),
            ));
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
            panic!("unexpected upload");
        }
    }
    let input = settings(
        "donbot",
        "donbot-test-personal",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
    )
    .unwrap();
    let guest = Guest::default();
    deliver_settings(&input, &guest, ActivationPolicy::DeferConsumer).unwrap();
    let calls = guest.0.borrow();
    assert_eq!(calls.len(), 2);
    for (args, settings) in calls.iter() {
        assert!(args.iter().any(|v| v == settings.instance.credential()));
        assert!(args.iter().any(|v| v == settings.instance.initial_unit()));
        assert!(args.iter().any(|v| v == "--no-start"));
        assert_eq!(settings.environment, "test");
    }
    assert_eq!(
        calls[0].1.device_name,
        "donbot-test-personal.example.invalid"
    );
    assert_eq!(
        calls[1].1.device_name,
        "donbot-test-personal-personal.example.invalid"
    );
}
