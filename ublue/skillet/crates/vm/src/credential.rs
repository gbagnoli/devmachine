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
    if activation == ActivationPolicy::DeferConsumer {
        return transport.stage_credentials(host, consumer_unit, &[(credential, payload)]);
    }
    send_one(
        transport,
        host,
        credential,
        consumer_unit,
        activation,
        payload,
    )
}

fn send_one(
    transport: &(impl GuestTransport + ?Sized),
    host: &str,
    credential: &str,
    consumer_unit: &str,
    activation: ActivationPolicy,
    payload: &[u8],
) -> Result<()> {
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
    if activation == ActivationPolicy::DeferConsumer {
        return transport.stage_credentials(host, consumer_unit, credentials);
    }
    send_set(transport, host, consumer_unit, activation, credentials)
}

pub(crate) fn send_set(
    transport: &(impl GuestTransport + ?Sized),
    host: &str,
    consumer_unit: &str,
    activation: ActivationPolicy,
    credentials: &[(&str, &[u8])],
) -> Result<()> {
    if credentials.len() > 1
        && send_batch(
            transport,
            host,
            consumer_unit,
            activation,
            credentials,
            None,
        )?
    {
        return Ok(());
    }
    for (name, payload) in credentials {
        send_one(transport, host, name, consumer_unit, activation, payload)?;
    }
    Ok(())
}

fn send_batch(
    transport: &(impl GuestTransport + ?Sized),
    host: &str,
    consumer_unit: &str,
    activation: ActivationPolicy,
    credentials: &[(&str, &[u8])],
    runner: Option<&str>,
) -> Result<bool> {
    let binary = format!("/var/usrlocal/bin/skillet-{host}");
    let mut args = if let Some(path) = runner {
        vec!["-n", path, "credential-batch", consumer_unit]
    } else {
        vec!["-n", &binary, "credential", "install-batch", consumer_unit]
    };
    if runner.is_none() && activation == ActivationPolicy::DeferConsumer {
        args.push("--no-start");
    }
    let payload = zeroize::Zeroizing::new(
        serde_json::to_vec(credentials)
            .map_err(|_| Error::Invalid("could not encode credential batch".into()))?,
    );
    let output = transport.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &args,
        },
        Some(&payload),
    )?;
    if output.status.success() {
        return Ok(true);
    }
    // Older image-owned binaries do not recognize the subcommand.
    if output.status.code() != Some(2)
        || !String::from_utf8_lossy(&output.stderr)
            .contains("unrecognized subcommand 'install-batch'")
    {
        return Err(Error::Guest {
            operation: "credential batch installation".into(),
            code: output.status.code(),
        });
    }
    Ok(false)
}

type Pending = (String, String, zeroize::Zeroizing<Vec<u8>>);

/// Collect a staging wave in memory while forwarding credential state checks
/// and noncredential operations to the same checked guest transport.
pub struct Deferred<'a, T> {
    guest: &'a T,
    host: &'a str,
    runner: Option<&'a str>,
    pending: std::cell::RefCell<Vec<Pending>>,
}

impl<'a, T: GuestTransport> Deferred<'a, T> {
    pub fn new(guest: &'a T, host: &'a str) -> Self {
        Self {
            guest,
            host,
            runner: None,
            pending: std::cell::RefCell::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn using_runner(mut self, path: Option<&'a str>) -> Self {
        self.runner = path;
        self
    }

    pub fn flush(self) -> Result<()> {
        let pending = self.pending.into_inner();
        let Some((unit, _, _)) = pending.first() else {
            return Ok(());
        };
        let credentials = pending
            .iter()
            .map(|(_, name, value)| (name.as_str(), value.as_slice()))
            .collect::<Vec<_>>();
        // The batch never starts a consumer. Older guests retain the correct
        // unit per credential even when using the fallback protocol.
        if (credentials.len() > 1 || self.runner.is_some())
            && send_batch(
                self.guest,
                self.host,
                unit,
                ActivationPolicy::DeferConsumer,
                &credentials,
                self.runner,
            )?
        {
            return Ok(());
        }
        for (unit, name, value) in &pending {
            send_one(
                self.guest,
                self.host,
                name,
                unit,
                ActivationPolicy::DeferConsumer,
                value,
            )?;
        }
        Ok(())
    }
}

impl<T: GuestTransport> GuestTransport for Deferred<'_, T> {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> Result<std::process::Output> {
        self.guest.execute(command, input)
    }

    fn upload(&self, source: &std::path::Path, destination: &str) -> Result<()> {
        self.guest.upload(source, destination)
    }

    fn stage_credentials(
        &self,
        host: &str,
        unit: &str,
        credentials: &[(&str, &[u8])],
    ) -> Result<()> {
        if host != self.host {
            return Err(Error::Invalid("credential batch target mismatch".into()));
        }
        let mut pending = self.pending.borrow_mut();
        for &(name, value) in credentials {
            if pending.iter().any(|(_, existing, _)| existing == name) {
                return Err(Error::Invalid(
                    "duplicate credential in staging wave".into(),
                ));
            }
            pending.push((
                unit.to_owned(),
                name.to_owned(),
                zeroize::Zeroizing::new(value.to_vec()),
            ));
        }
        Ok(())
    }
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
