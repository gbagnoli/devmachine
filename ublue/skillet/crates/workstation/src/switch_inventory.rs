//! Validated, workstation-resolved NETGEAR switch inventory.
//!
//! Device passwords are held only in the in-memory delivery object and are
//! intentionally excluded from `Debug` output and validation errors.
use crate::provisioning_policy::ProvisioningPolicy;
use serde::{Deserialize, Serialize};
use skillet_hosts::HostProfile;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use url::Url;

const MAX_SWITCHES: usize = 32;

#[derive(Debug, Error)]
pub enum SwitchInventoryError {
    #[error("invalid switch inventory or credential entry")]
    Invalid,
    #[error("invalid switch list: {0}")]
    InvalidSwitchList(&'static str),
    #[error("a switch is assigned to more than one production collector host")]
    DuplicateOwner,
    #[error(transparent)]
    Vault(#[from] crate::vault::VaultError),
    #[error(transparent)]
    Datadog(#[from] skillet_datadog::DatadogError),
}

/// Attach a validated inventory to a normal Datadog configuration payload.
pub fn add_to_datadog_payload(
    input: &str,
    selection: SwitchSelection,
    kexec_enabled: bool,
) -> Result<String, SwitchInventoryError> {
    let mut input = skillet_datadog::Input::parse(input)?;
    let switches = selection.map(|switches| {
        switches
            .into_iter()
            .map(|switch| skillet_datadog::SwitchConfig {
                name: switch.name,
                endpoint: switch.endpoint,
                make: switch.make,
                password: switch.password,
                metrics_port: switch.metrics_port,
            })
            .collect()
    });
    input.set_switches(switches)?;
    Ok(input.render_with_kexec(kexec_enabled)?)
}

/// A single credential-bearing switch record. Do not add `Debug` or `Display`.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchCredential {
    pub name: String,
    pub endpoint: String,
    pub make: String,
    pub password: String,
    pub metrics_port: u16,
}

/// `None` means no inventory field is configured; `Some([])` is an explicit
/// request to remove all previously managed collectors.
pub type SwitchSelection = Option<Vec<SwitchCredential>>;

impl SwitchCredential {
    fn validate(&self) -> Result<(), SwitchInventoryError> {
        if !valid_name(&self.name)
            || self.make != "netgear"
            || self.password.is_empty()
            || self.password.contains('\0')
            || !valid_endpoint(&self.endpoint)
        {
            return Err(SwitchInventoryError::Invalid);
        }
        Ok(())
    }
}

/// Read the host inventory and referenced per-device entries through one
/// workstation-owned field lookup. Test policy never reads production entries.
pub fn resolve(
    host: &str,
    environment: &str,
    lookup: impl Fn(&str, &str) -> Result<Option<String>, crate::vault::VaultError>,
) -> Result<SwitchSelection, SwitchInventoryError> {
    if !valid_name(host) || !matches!(environment, "prod" | "test") {
        return Err(SwitchInventoryError::Invalid);
    }
    if environment == "test" {
        return Ok(None);
    }

    if !skillet_hosts::profile_for_name(host)
        .is_some_and(|profile| profile.supports_service("datadog"))
    {
        return Err(SwitchInventoryError::Invalid);
    }

    // Read inventory names for every Datadog host to enforce one owner without
    // loading another host's device passwords into this provisioning process.
    let mut inventories = Vec::new();
    for profile in skillet_hosts::declared_profiles()
        .iter()
        .filter(|profile| profile.supports_service("datadog"))
    {
        let owner = profile.id.as_str().to_string();
        let path = format!("skillet/hosts/{owner}/network/switches");
        if let Some(value) = lookup(&path, "switches")? {
            inventories.push((owner, parse_names(&value)?));
        }
    }
    validate_inventory_owners(&inventories)?;
    let Some(names) = inventories
        .iter()
        .find(|(owner, _)| owner == host)
        .map(|(_, names)| names.clone())
    else {
        return Ok(None);
    };
    if names.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let mut credentials = Vec::with_capacity(names.len());
    let mut names = names;
    names.sort();
    for (index, name) in names.into_iter().enumerate() {
        let entry_path = format!("skillet/network/switches/{name}");
        let make = lookup(&entry_path, "make")?.ok_or(SwitchInventoryError::Invalid)?;
        let endpoint = lookup(&entry_path, "URL")?.ok_or(SwitchInventoryError::Invalid)?;
        let password = lookup(&entry_path, "Password")?.ok_or(SwitchInventoryError::Invalid)?;
        let credential = SwitchCredential {
            name,
            endpoint,
            make,
            password,
            metrics_port: 9500 + u16::try_from(index).map_err(|_| SwitchInventoryError::Invalid)?,
        };
        credential.validate()?;
        credentials.push(credential);
    }
    Ok(Some(credentials))
}

/// Parse comma- or newline-separated stable names. An actually empty field
/// deliberately means “remove all managed collectors”.
pub fn parse_names(value: &str) -> Result<Vec<String>, SwitchInventoryError> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for component in value.split([',', '\n']) {
        let name = component.trim();
        if name.is_empty() {
            return Err(SwitchInventoryError::InvalidSwitchList(
                "remove empty items, including a trailing comma",
            ));
        }
        if !valid_name(name) {
            return Err(SwitchInventoryError::InvalidSwitchList(
                "use 1–32 lowercase ASCII letters, digits, hyphens or underscores; start with a letter and end with a letter or digit",
            ));
        }
        if !seen.insert(name.to_string()) {
            return Err(SwitchInventoryError::InvalidSwitchList(
                "each switch name must appear only once",
            ));
        }
        names.push(name.to_string());
    }
    if names.len() > MAX_SWITCHES {
        return Err(SwitchInventoryError::InvalidSwitchList(
            "list at most 32 switches",
        ));
    }
    Ok(names)
}

/// Enforce a single collector owner for each stable device name.
pub fn validate_single_owner<'a>(
    inventories: impl IntoIterator<Item = (&'a str, &'a [SwitchCredential])>,
) -> Result<(), SwitchInventoryError> {
    let mut owners = std::collections::BTreeMap::<&str, &str>::new();
    for (host, switches) in inventories {
        for switch in switches {
            if owners.insert(&switch.name, host).is_some() {
                return Err(SwitchInventoryError::DuplicateOwner);
            }
        }
    }
    Ok(())
}

pub struct InventoryProblem {
    pub path: String,
    pub fields: Vec<&'static str>,
    pub invalid: bool,
    pub detail: Option<&'static str>,
}

pub struct InventoryAudit {
    pub checked: usize,
    pub present: Vec<String>,
    pub missing: Vec<InventoryProblem>,
}

/// Audit only explicitly present production inventories. Missing inventory is
/// disabled; test policy returns without reading production values.
pub fn audit(
    selected_profiles: &[HostProfile],
    all_profiles: &[HostProfile],
    policy: ProvisioningPolicy,
    stored: &[String],
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, crate::vault::VaultError>,
) -> Result<InventoryAudit, SwitchInventoryError> {
    let mut audit = InventoryAudit {
        checked: 0,
        present: Vec::new(),
        missing: Vec::new(),
    };
    if policy.vault_name() != "prod" {
        return Ok(audit);
    }

    let mut inventories = BTreeMap::<String, (Vec<String>, String)>::new();
    let selected_hosts = selected_profiles
        .iter()
        .map(|profile| profile.id.as_str())
        .collect::<BTreeSet<_>>();
    for profile in all_profiles
        .iter()
        .filter(|profile| profile.supports_service("datadog"))
    {
        let host = profile.id.as_str();
        let path = format!("skillet/hosts/{host}/network/switches");
        if !stored.iter().any(|stored_path| stored_path == &path) {
            continue;
        }
        let Some(names) = lookup(&path, "switches")? else {
            continue;
        };
        if selected_hosts.contains(host) {
            audit.checked += 1;
        }
        let parsed = match parse_names(&names) {
            Ok(names) => names,
            Err(SwitchInventoryError::InvalidSwitchList(detail)) => {
                if selected_profiles
                    .iter()
                    .any(|selected| selected.id.as_str() == host)
                {
                    audit.missing.push(InventoryProblem {
                        path,
                        fields: vec!["switches"],
                        invalid: true,
                        detail: Some(detail),
                    });
                }
                continue;
            }
            Err(error) => return Err(error),
        };
        if selected_hosts.contains(host) {
            audit.present.push(path.clone());
        }
        inventories.insert(host.into(), (parsed, path));
    }

    let mut owners = BTreeMap::<&str, &str>::new();
    let mut duplicate_names = BTreeSet::new();
    for (host, (names, _)) in &inventories {
        for name in names {
            if owners.insert(name, host).is_some() {
                duplicate_names.insert(name.as_str());
            }
        }
    }

    for profile in selected_profiles
        .iter()
        .filter(|profile| profile.supports_service("datadog"))
    {
        let host = profile.id.as_str();
        let Some((names, inventory_path)) = inventories.get(host) else {
            continue;
        };
        if names
            .iter()
            .any(|name| duplicate_names.contains(name.as_str()))
        {
            audit.missing.push(InventoryProblem {
                path: inventory_path.clone(),
                fields: vec!["switches"],
                invalid: true,
                detail: Some("a switch name is assigned to more than one production host"),
            });
            continue;
        }
        for name in names {
            let path = format!("skillet/network/switches/{name}");
            audit.checked += 1;
            let (fields, invalid) = audit_device_fields(&path, lookup)?;
            if fields.is_empty() {
                audit.present.push(path);
            } else {
                audit.missing.push(InventoryProblem {
                    path,
                    fields,
                    invalid,
                    detail: None,
                });
            }
        }
    }
    Ok(audit)
}

fn audit_device_fields(
    path: &str,
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, crate::vault::VaultError>,
) -> Result<(Vec<&'static str>, bool), SwitchInventoryError> {
    let make = lookup(path, "make")?;
    let endpoint = lookup(path, "URL")?;
    let password = lookup(path, "Password")?;
    let mut fields = Vec::new();
    let mut invalid = false;
    match &make {
        Some(value) if value == "netgear" => {}
        Some(_) => {
            fields.push("make");
            invalid = true;
        }
        None => fields.push("make"),
    }
    match &endpoint {
        Some(value) if valid_endpoint(value) => {}
        Some(_) => {
            fields.push("URL");
            invalid = true;
        }
        None => fields.push("URL"),
    }
    match &password {
        Some(value) if !value.is_empty() && !value.contains('\0') => {}
        Some(_) => {
            fields.push("Password");
            invalid = true;
        }
        None => fields.push("Password"),
    }
    Ok((fields, invalid))
}

/// Dynamic device entry names referenced by inventories. Invalid inventory
/// does not authorize treating unrelated vault entries as used.
pub fn referenced_entry_paths(
    profiles: &[HostProfile],
    policy: ProvisioningPolicy,
    stored: &[String],
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, crate::vault::VaultError>,
) -> Result<BTreeSet<String>, SwitchInventoryError> {
    let mut paths = BTreeSet::new();
    if policy.vault_name() != "prod" {
        return Ok(paths);
    }
    for profile in profiles
        .iter()
        .filter(|profile| profile.supports_service("datadog"))
    {
        let path = format!("skillet/hosts/{}/network/switches", profile.id.as_str());
        if stored.iter().any(|stored_path| stored_path == &path) {
            paths.insert(path.clone());
        }
        let Some(value) = lookup(&path, "switches")? else {
            continue;
        };
        paths.insert(path);
        if let Ok(names) = parse_names(&value) {
            paths.extend(
                names
                    .into_iter()
                    .map(|name| format!("skillet/network/switches/{name}")),
            );
        }
    }
    Ok(paths)
}

/// Return dynamic checklist items for configured switch inventories. Inventory
/// existence opts a host into the module; an absent/blank inventory requires
/// no device fields. Only the selected environment is examined.
pub fn requirement_problems(
    profiles: &[HostProfile],
    all_profiles: &[HostProfile],
    policy: ProvisioningPolicy,
    stored: &[String],
    lookup: &impl Fn(&str, &str) -> Result<Option<String>, crate::vault::VaultError>,
) -> Result<Vec<InventoryProblem>, SwitchInventoryError> {
    Ok(audit(profiles, all_profiles, policy, stored, lookup)?.missing)
}

fn validate_inventory_owners(
    inventories: &[(String, Vec<String>)],
) -> Result<(), SwitchInventoryError> {
    let mut owners = std::collections::BTreeMap::<&str, &str>::new();
    for (host, names) in inventories {
        for name in names {
            if owners.insert(name, host).is_some() {
                return Err(SwitchInventoryError::DuplicateOwner);
            }
        }
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        && name
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn valid_endpoint(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    // py-netgear-plus currently speaks HTTP to these switches. Do not imply
    // HTTPS transport support merely because an HTTPS URL parses. Requiring a
    // private/link-local literal keeps the cleartext credential on the LAN.
    let private_host = match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_private() || address.is_link_local(),
        Some(url::Host::Ipv6(address)) => {
            address.is_unique_local() || address.is_unicast_link_local()
        }
        Some(url::Host::Domain(_)) | None => false,
    };
    url.scheme() == "http"
        && private_host
        && url.username().is_empty()
        && url.password().is_none()
        && (url.path().is_empty() || url.path() == "/")
        && url.query().is_none()
        && url.fragment().is_none()
}

#[cfg(test)]
#[path = "switch_inventory/tests.rs"]
mod tests;
