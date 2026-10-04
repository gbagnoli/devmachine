//! Shared, stdin-only installation of encrypted guest credentials.

use crate::{
    transport::{GuestCommand, GuestTransport},
    Error, Result,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationPolicy {
    StartConsumer,
    DeferConsumer,
}

pub fn install(
    transport: &impl GuestTransport,
    host: &str,
    credential: &str,
    consumer_unit: &str,
    activation: ActivationPolicy,
    payload: &[u8],
) -> Result<()> {
    validate_input(host, credential, consumer_unit, payload)?;

    let binary = format!("/var/usrlocal/bin/skillet-{host}");
    let mut arguments = vec!["-n", binary.as_str(), "credential", "install"];
    arguments.push(credential);
    arguments.push(consumer_unit);
    if activation == ActivationPolicy::DeferConsumer {
        arguments.push("--no-start");
    }
    let output = transport.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &arguments,
        },
        Some(payload),
    )?;
    if !output.status.success() {
        return Err(Error::Guest {
            operation: "credential installation".into(),
            code: output.status.code(),
        });
    }
    Ok(())
}

/// Install a related credential set using one activation policy. Every input
/// is validated before any guest mutation; deferred callers can start the
/// consumer only after this operation succeeds.
pub fn install_set(
    transport: &impl GuestTransport,
    host: &str,
    consumer_unit: &str,
    activation: ActivationPolicy,
    credentials: &[(&str, &[u8])],
) -> Result<()> {
    if credentials.is_empty() {
        return Err(Error::Invalid(
            "guest credential delivery set is empty".into(),
        ));
    }
    for (index, (name, payload)) in credentials.iter().enumerate() {
        validate_input(host, name, consumer_unit, payload)?;
        if credentials[..index]
            .iter()
            .any(|(previous, _)| previous == name)
        {
            return Err(Error::Invalid(
                "guest credential delivery set contains duplicate names".into(),
            ));
        }
    }
    for (name, payload) in credentials {
        install(transport, host, name, consumer_unit, activation, payload)?;
    }
    Ok(())
}

fn validate_input(host: &str, credential: &str, consumer_unit: &str, payload: &[u8]) -> Result<()> {
    if !valid_component(host)
        || !valid_component(credential)
        || !valid_unit(consumer_unit)
        || payload.is_empty()
    {
        return Err(Error::Invalid(
            "invalid guest credential delivery input".into(),
        ));
    }
    Ok(())
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_unit(value: &str) -> bool {
    value.ends_with(".service")
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@'))
}

#[cfg(test)]
#[path = "credential/tests.rs"]
mod tests;
