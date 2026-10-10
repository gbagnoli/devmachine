//! Shared SMTP payload construction; test policy never reads provider credentials.
use crate::{
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::VaultError,
};
use skillet_smtp::{Input, SmtpError};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing SMTP vault field: {0}")]
    Missing(&'static str),
    #[error("host does not declare SMTP")]
    Host,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Config(#[from] SmtpError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
}

pub fn input(
    policy: ProvisioningPolicy,
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, VaultError>,
) -> Result<Input, Error> {
    if policy.environment() == Environment::Test {
        return Ok(Input::Capture {});
    }
    let field = |name| lookup("skillet/smtp", name)?.ok_or(Error::Missing(name));
    let port = field("port")?
        .parse::<u16>()
        .map_err(|_| SmtpError::Invalid)?;
    let input = Input::Production {
        host: field("host")?,
        port,
        tls: field("tls")?,
        username: field("UserName")?,
        password: field("Password")?,
        sender: field("sender")?,
    };
    input.validate()?;
    Ok(input)
}

pub fn deliver(host: &str, input: &Input, guest: &impl GuestTransport) -> Result<(), Error> {
    let profile = skillet_hosts::profile_for_name(host)
        .filter(|p| p.supports_service("smtp"))
        .ok_or(Error::Host)?;
    credential::install(
        guest,
        profile.id.as_str(),
        skillet_smtp::CREDENTIAL,
        "skillet-smtp-apply.service",
        ActivationPolicy::StartConsumer,
        input.payload()?.as_bytes(),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "smtp_provisioning/tests.rs"]
mod tests;
