//! Environment-scoped DNS inventory; no topology values are baked into images.
use crate::{
    cloudflare::Cloudflare,
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::{Vault, VaultError},
};
use skillet_pihole::dns::{Input, Record};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing or invalid private DNS configuration at {0}; see the Pi-hole README")]
    Entry(String),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Config(#[from] skillet_pihole::dns::Error),
    #[error(transparent)]
    Cloudflare(#[from] crate::cloudflare::CloudflareError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
}
pub fn prefix(policy: ProvisioningPolicy) -> String {
    format!("skillet/environments/{}/dns", policy.vault_name())
}
fn validate_collection_paths(root: &str, paths: &[String]) -> Result<(), Error> {
    // A misspelled collection group must not silently become an empty inventory.
    for path in paths {
        if let Some(relative) = path.strip_prefix(&format!("{root}/")) {
            if relative
                .split('/')
                .next()
                .is_some_and(|group| group.trim() == "entries" && group != "entries")
            {
                return Err(Error::Entry(path.clone()));
            }
        }
    }
    Ok(())
}

pub fn input(
    policy: ProvisioningPolicy,
    zone: &str,
    paths: &[String],
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, VaultError>,
) -> Result<Input, Error> {
    let root = prefix(policy);
    let base_path = format!("{root}/local-prefix");
    let relative =
        lookup(&base_path, "Password")?.ok_or_else(|| Error::Entry(base_path.clone()))?;
    if !skillet_pihole::dns::valid_name(&relative) || !skillet_pihole::dns::valid_name(zone) {
        return Err(Error::Entry(base_path));
    }
    let base = format!("{relative}.{zone}");
    let ui_path = policy.ui_domain_entry();
    let ui_prefix = lookup(&ui_path, "Password")?;
    let ui_domain = skillet_caddy::resolve_ui_domain(zone, ui_prefix.as_deref())
        .map_err(|_| Error::Entry(ui_path))?
        .to_ascii_lowercase();
    let entries = format!("{root}/entries/");
    let mut records = Vec::new();
    let mut paths = paths.to_vec();
    paths.sort();
    validate_collection_paths(&root, &paths)?;
    for path in paths.iter().filter(|p| p.starts_with(&entries)) {
        let title = path
            .strip_prefix(&entries)
            .ok_or_else(|| Error::Entry(path.clone()))?;
        if title.contains('/') || !skillet_pihole::dns::valid_name(title) {
            return Err(Error::Entry(path.clone()));
        }
        let raw_names = lookup(path, "names")?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| title.into());
        let names = raw_names
            .split([',', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|name| {
                if !skillet_pihole::dns::valid_name(name) {
                    return Err(Error::Entry(path.clone()));
                }
                Ok(format!("{name}.{base}"))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut addresses = Vec::new();
        if let Some(v) = lookup(path, "ipv4")?.filter(|s| !s.trim().is_empty()) {
            addresses.push(IpAddr::V4(
                v.trim()
                    .parse::<Ipv4Addr>()
                    .map_err(|_| Error::Entry(path.clone()))?,
            ));
        }
        if let Some(v) = lookup(path, "ipv6")?.filter(|s| !s.trim().is_empty()) {
            addresses.push(IpAddr::V6(
                v.trim()
                    .parse::<Ipv6Addr>()
                    .map_err(|_| Error::Entry(path.clone()))?,
            ));
        }
        records.push(Record { names, addresses });
    }
    // Physical addresses never enter disposable test configuration. Every production
    // resolver receives the same generated inventory, not just its own host record.
    if policy.environment() == Environment::Production {
        for profile in skillet_hosts::declared_profiles() {
            let Some(selection) = profile.host_network_selection() else {
                continue;
            };
            let host = profile.id.as_str();
            for (kind, selected) in [("wired", selection.wired), ("wifi", selection.wifi)] {
                if !selected {
                    continue;
                }
                let path = format!("skillet/hosts/{host}/network/{kind}");
                let ip = lookup(&path, "address")?
                    .ok_or_else(|| Error::Entry(path.clone()))?
                    .parse::<Ipv4Addr>()
                    .map_err(|_| Error::Entry(path))?;
                let name = if kind == "wired" || !selection.wired {
                    host.into()
                } else {
                    format!("{host}-wifi")
                };
                records.push(Record {
                    names: vec![format!("{name}.{base}")],
                    addresses: vec![ip.into()],
                });
            }
        }
    }
    let input = Input {
        version: 1,
        records,
    };
    input.validate()?;
    if input.records.iter().flat_map(|r| &r.names).any(|name| {
        name.eq_ignore_ascii_case(&ui_domain)
            || name
                .to_ascii_lowercase()
                .ends_with(&format!(".{ui_domain}"))
    }) {
        return Err(Error::Entry(entries));
    }
    Ok(input)
}

pub fn deliver(
    host: &str,
    policy: ProvisioningPolicy,
    vault: &Vault,
    guest: &impl skillet_vm::transport::GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<(), Error> {
    if !skillet_hosts::profile_for_name(host).is_some_and(|p| p.supports_service("pihole")) {
        return Err(Error::Entry("host does not select Pi-hole".into()));
    }
    let zone_id = vault
        .get(&policy.cloudflare_zone_entry())?
        .ok_or_else(|| Error::Entry(policy.cloudflare_zone_entry()))?;
    let token = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| Error::Entry("skillet/cloudflare/token-creator".into()))?;
    let zone = Cloudflare::new().zone(&token, &zone_id)?;
    if zone.id != zone_id {
        return Err(Error::Entry(policy.cloudflare_zone_entry()));
    }
    let input = input(policy, &zone.name, &vault.entry_paths(), &|path, field| {
        vault.get_field(path, field)
    })?;
    skillet_vm::credential::install(
        guest,
        host,
        skillet_pihole::dns::CREDENTIAL,
        skillet_pihole::dns::UNIT,
        activation,
        input.payload()?.as_bytes(),
    )?;
    Ok(())
}
#[cfg(test)]
#[path = "pihole_dns/tests.rs"]
mod tests;
