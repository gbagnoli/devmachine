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
    if !valid_component(host)
        || !valid_component(credential)
        || !valid_unit(consumer_unit)
        || payload.is_empty()
    {
        return Err(Error::Invalid(
            "invalid guest credential delivery input".into(),
        ));
    }

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
