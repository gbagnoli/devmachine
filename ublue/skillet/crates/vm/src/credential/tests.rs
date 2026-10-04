use super::{install, install_set, ActivationPolicy};
use crate::{
    transport::{GuestCommand, GuestTransport},
    Error, Result,
};
use std::{
    path::Path,
    process::{Command, Output},
};

type CapturedCall = (String, Vec<String>, Option<Vec<u8>>);

#[derive(Default)]
struct FakeTransport {
    calls: std::sync::Mutex<Vec<CapturedCall>>,
    fail: bool,
}

impl GuestTransport for FakeTransport {
    fn execute(&self, command: &GuestCommand<'_>, input: Option<&[u8]>) -> Result<Output> {
        self.calls.lock().unwrap().push((
            command.program.to_string(),
            command
                .arguments
                .iter()
                .map(|arg| (*arg).to_string())
                .collect(),
            input.map(<[u8]>::to_vec),
        ));
        Ok(Output {
            status: Command::new(if self.fail { "false" } else { "true" })
                .status()
                .map_err(Error::Io)?,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn upload(&self, _source: &Path, _destination: &str) -> Result<()> {
        Ok(())
    }
}

#[test]
fn sends_payload_only_as_stdin_and_defers_activation_when_requested() {
    let transport = FakeTransport::default();
    install(
        &transport,
        "clamps",
        "cloudflare_acme_token",
        "skillet-caddy-apply.service",
        ActivationPolicy::DeferConsumer,
        b"secret bytes",
    )
    .unwrap();
    let calls = transport.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/usr/bin/sudo");
    assert_eq!(calls[0].1.last().map(String::as_str), Some("--no-start"));
    assert_eq!(calls[0].2.as_deref(), Some(b"secret bytes".as_slice()));
    assert!(!calls[0].1.iter().any(|argument| argument == "secret bytes"));
}

#[test]
fn rejects_invalid_names_and_empty_payload_before_transport() {
    let transport = FakeTransport::default();
    for (host, credential, unit, payload) in [
        ("clamps;id", "token", "unit.service", b"secret".as_slice()),
        ("clamps", "token;id", "unit.service", b"secret".as_slice()),
        ("clamps", "token", "unit", b"secret".as_slice()),
        ("clamps", "token", "unit.service", b"".as_slice()),
    ] {
        assert!(install(
            &transport,
            host,
            credential,
            unit,
            ActivationPolicy::StartConsumer,
            payload,
        )
        .is_err());
    }
    assert!(transport.calls.lock().unwrap().is_empty());
}

#[test]
fn credential_set_validates_all_entries_then_defers_each_install() {
    let transport = FakeTransport::default();
    install_set(
        &transport,
        "clamps",
        "skillet-caddy-apply.service",
        ActivationPolicy::DeferConsumer,
        &[
            ("caddy_sites", b"sites payload"),
            ("cloudflare_acme_token", b"token payload"),
        ],
    )
    .unwrap();
    let calls = transport.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls
        .iter()
        .all(|call| call.1.last().map(String::as_str) == Some("--no-start")));
    assert_eq!(calls[0].2.as_deref(), Some(b"sites payload".as_slice()));
    assert_eq!(calls[1].2.as_deref(), Some(b"token payload".as_slice()));
}

#[test]
fn credential_set_rejects_invalid_or_duplicate_entries_before_any_mutation() {
    let transport = FakeTransport::default();
    for credentials in [
        vec![("caddy_sites", b"sites".as_slice()), ("bad;name", b"token")],
        vec![
            ("caddy_sites", b"sites".as_slice()),
            ("caddy_sites", b"again"),
        ],
    ] {
        assert!(install_set(
            &transport,
            "clamps",
            "skillet-caddy-apply.service",
            ActivationPolicy::DeferConsumer,
            &credentials,
        )
        .is_err());
    }
    assert!(transport.calls.lock().unwrap().is_empty());
}

#[test]
fn reports_guest_failure_without_exposing_payload() {
    let transport = FakeTransport {
        fail: true,
        ..FakeTransport::default()
    };
    let error = install(
        &transport,
        "clamps",
        "pihole_web_password",
        "skillet-full-apply.service",
        ActivationPolicy::StartConsumer,
        b"private value",
    )
    .unwrap_err();
    assert!(error.to_string().contains("credential installation"));
    assert!(!error.to_string().contains("private value"));
}
