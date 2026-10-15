//! Validated private records and a narrow Pi-hole v6 configuration mutation.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skillet_core::{
    activation::{self, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::{collections::BTreeSet, net::IpAddr, path::Path};
use thiserror::Error;

pub const CREDENTIAL: &str = "pihole_dns";
pub const UNIT: &str = "skillet-pihole-dns-apply.service";
const CONFIG: &str = "/var/lib/data/pihole/etc/pihole.toml";
const STATE: &str = "/var/lib/skillet/pihole-dns";

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid private DNS configuration; check names, address families and conflicts")]
    Invalid,
    #[error("Pi-hole v6 configuration is missing or invalid; apply the declared container first")]
    Configuration,
    #[error(transparent)]
    File(#[from] skillet_core::files::FileError),
    #[error(transparent)]
    System(#[from] skillet_core::system::SystemError),
    #[error(transparent)]
    Activation(#[from] activation::ActivationError),
}

// Private topology deliberately has no Debug representation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub names: Vec<String>,
    pub addresses: Vec<IpAddr>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub version: u8,
    pub records: Vec<Record>,
}

pub fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.parse::<IpAddr>().is_err()
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
pub fn valid_address(ip: IpAddr) -> bool {
    !ip.is_unspecified() && !ip.is_multicast() && !matches!(ip, IpAddr::V4(v) if v.is_broadcast())
}
impl Input {
    pub fn parse(payload: &str) -> Result<Self, Error> {
        let input: Self = serde_json::from_str(payload).map_err(|_| Error::Invalid)?;
        input.validate()?;
        Ok(input)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let mut names = BTreeSet::new();
        if self.version != 1 {
            return Err(Error::Invalid);
        }
        for record in &self.records {
            if record.names.is_empty()
                || record.addresses.is_empty()
                || record
                    .names
                    .iter()
                    .any(|name| !valid_name(name) || !names.insert(name.to_ascii_lowercase()))
                || record.addresses.iter().any(|ip| !valid_address(*ip))
                || record.addresses.iter().collect::<BTreeSet<_>>().len() != record.addresses.len()
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn payload(&self) -> Result<String, Error> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| Error::Invalid)
    }
    pub fn hosts(&self) -> Result<Vec<String>, Error> {
        self.validate()?;
        let mut lines = Vec::new();
        for record in &self.records {
            let names = record
                .names
                .iter()
                .map(|s| s.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            for ip in &record.addresses {
                lines.push(format!("{ip} {names}"));
            }
        }
        lines.sort();
        Ok(lines)
    }
}

pub(super) fn render(bytes: &[u8], hosts: &[String]) -> Result<(String, bool), Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Configuration)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| Error::Configuration)?;
    if !document.get("dns").is_some_and(toml_edit::Item::is_table) {
        return Err(Error::Configuration);
    }
    let current = document["dns"].get("hosts");
    let equal = current
        .and_then(toml_edit::Item::as_array)
        .is_some_and(|array| {
            array
                .iter()
                .map(toml_edit::Value::as_str)
                .collect::<Vec<_>>()
                == hosts.iter().map(|s| Some(s.as_str())).collect::<Vec<_>>()
        });
    if equal {
        return Ok((text.into(), false));
    }
    let mut array = toml_edit::Array::new();
    for line in hosts {
        array.push(line.as_str());
    }
    document["dns"]["hosts"] = toml_edit::value(array);
    Ok((document.to_string(), true))
}

/// Only dns.hosts is owned here. Stop the consumer before re-reading/mutating
/// its file, retain comments/other settings, and persist activation after success.
pub fn apply<S, F>(system: &S, files: &F, input: &Input) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let hosts = input.hosts()?;
    let bytes = files
        .read_file(Path::new(CONFIG))?
        .ok_or(Error::Configuration)?;
    let (_, changed) = render(&bytes, &hosts)?;
    if changed {
        system.service_stop("pihole.service")?;
        let bytes = files
            .read_file(Path::new(CONFIG))?
            .ok_or(Error::Configuration)?;
        let (text, _) = render(&bytes, &hosts)?;
        files.ensure_file(
            Path::new(CONFIG),
            text.as_bytes(),
            None,
            &Ownership::default(),
        )?;
    }
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new(STATE), Some(0o700), &root)?;
    let revision = Sha256::digest(serde_json::to_vec(&hosts).map_err(|_| Error::Invalid)?).to_vec();
    activation::activate(
        system,
        files,
        &ActivationRequest {
            service: "pihole.service".into(),
            state_path: Path::new(STATE).join("applied"),
            revision,
            definition_changed: changed,
            reload_daemon: false,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "dns/tests.rs"]
mod tests;
