//! Vault fields become a typed native networking credential, never CLI arguments.
use crate::{
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::VaultError,
};
use skillet_network::{Input, Interface, NetworkError, Wireless};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "native networking delivery is production-only and requires caller-selected interfaces"
    )]
    Policy,
    #[error("missing network vault field {field} in {path}")]
    Missing { path: String, field: &'static str },
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Config(#[from] NetworkError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
}

pub fn input(
    host: &str,
    policy: ProvisioningPolicy,
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, VaultError>,
) -> Result<Input, Error> {
    // Reject test delivery before reading any physical deployment fields.
    if policy.environment() != Environment::Production {
        return Err(Error::Policy);
    }
    let selection = skillet_hosts::profile_for_name(host)
        .and_then(|profile| profile.host_network_selection())
        .ok_or(Error::Policy)?;
    let read_interface = |kind: &str| {
        let path = format!("skillet/hosts/{host}/network/{kind}");
        let field = |name| {
            lookup(&path, name)?.ok_or_else(|| Error::Missing {
                path: path.clone(),
                field: name,
            })
        };
        let network = Interface {
            interface: field("interface")?,
            address: field("address")?,
            prefix: skillet_network::prefix_from_mask(&field("mask")?)
                .ok_or(NetworkError::Invalid)?,
            gateway: field("gateway")?,
            dns: [field("dns1")?, field("dns2")?],
        };
        Ok::<_, Error>(network)
    };
    let wired = selection
        .wired
        .then(|| read_interface("wired"))
        .transpose()?;
    let wifi = if selection.wifi {
        let path = format!("skillet/hosts/{host}/network/wifi");
        let field = |name| {
            lookup(&path, name)?.ok_or_else(|| Error::Missing {
                path: path.clone(),
                field: name,
            })
        };
        Some(Wireless {
            network: read_interface("wifi")?,
            ssid: field("UserName")?,
            password: field("Password")?,
        })
    } else {
        None
    };
    let input = Input {
        version: 1,
        host: host.into(),
        wired,
        wifi,
    };
    input.validate(host, selection)?;
    Ok(input)
}

pub fn deliver(
    host: &str,
    policy: ProvisioningPolicy,
    input: &Input,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), Error> {
    if policy.environment() != Environment::Production {
        return Err(Error::Policy);
    }
    let selection = skillet_hosts::profile_for_name(host)
        .and_then(|profile| profile.host_network_selection())
        .ok_or(Error::Policy)?;
    let payload = input.payload(host, selection)?;
    credential::install(
        guest,
        host,
        skillet_network::CREDENTIAL,
        skillet_network::PREPARE_UNIT,
        activation,
        payload.as_bytes(),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "network_provisioning/tests.rs"]
mod tests;
