//! Read-only vault requirement auditing and its shared human documentation.
use crate::{configuration_templates, provisioning_policy::ProvisioningPolicy, vault::VaultError};
use serde::Deserialize;
use skillet_hosts::HostProfile;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};
use thiserror::Error;

const CATALOG: &str = include_str!("secret-requirements.json");
const INTRO: &str = "# Skillet secrets checklist\n\nVault: `$XDG_DATA_HOME/skillet/secrets.kdbx` (default:\n`~/.local/share/skillet/secrets.kdbx`). Paths are **group path + entry title**.\nPut every value in **Password**, including IDs and configuration; leave\n**Username** empty. `<environment>` is `prod` or `test`; `<host>` is the\ncanonical host name. Only prepare entries for enabled modules.\n\nAudit with `skillet secrets check` (all declared hosts, production), or\n`skillet secrets check --host clamps --environment test`. It checks presence\nand nonempty Password fields, without provider calls or generating secrets.\nMissing optional/generated/planned entries do not fail the check.\n\n<!-- Generated from ublue/skillet/crates/workstation/src/secret-requirements.json. -->\n";

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("invalid secret requirements catalog: {0}")]
    Catalog(#[from] serde_json::Error),
    #[error("secret requirements catalog is inconsistent: {0}")]
    InvalidCatalog(String),
    #[error(transparent)]
    Template(#[from] configuration_templates::ConfigurationTemplateError),
    #[error(transparent)]
    Vault(#[from] VaultError),
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Required,
    Optional,
    Generated,
    Planned,
}

impl Status {
    const fn label(self) -> &'static str {
        match self {
            Self::Required => "Required",
            Self::Optional => "Optional",
            Self::Generated => "Generated",
            Self::Planned => "Planned",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementGroup {
    module: String,
    required_by: Vec<String>,
    description: String,
    template: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    status: Status,
    help: String,
}

fn catalog() -> Result<Vec<RequirementGroup>, CheckError> {
    let modules: Vec<RequirementGroup> = serde_json::from_str(CATALOG)?;
    if modules
        .windows(2)
        .any(|pair| pair[0].module.to_lowercase() >= pair[1].module.to_lowercase())
    {
        return Err(CheckError::InvalidCatalog(
            "modules must be alphabetical".into(),
        ));
    }
    Ok(modules)
}

/// Missing/invalid entry metadata only; never retains or renders secret values.
pub struct MissingEntry {
    pub module: String,
    pub path: String,
    pub guide: String,
    pub invalid: bool,
}

pub struct CheckReport {
    pub checked: usize,
    pub missing: Vec<MissingEntry>,
}

/// Find unreferenced Skillet entry paths across all supplied profiles and both
/// environments. Optional/generated entries and legacy readers count as used;
/// planned requirements do not. Personal vault entries are outside this audit.
pub fn unused_paths(
    profiles: &[HostProfile],
    stored: &[String],
) -> Result<Vec<String>, CheckError> {
    let mut used = BTreeSet::new();
    for module in catalog()? {
        for profile in profiles.iter().filter(|profile| selected(&module, profile)) {
            for environment in ["prod", "test"] {
                let references = module
                    .template
                    .as_deref()
                    .map(|service| {
                        configuration_templates::secret_paths(
                            service,
                            profile.id.as_str(),
                            environment,
                        )
                    })
                    .transpose()?;
                for entry in &module.entries {
                    if entry.status == Status::Planned {
                        continue;
                    }
                    let path = expand(&entry.path, profile.id.as_str(), environment);
                    if entry.status != Status::Required
                        || references
                            .as_ref()
                            .is_none_or(|paths| paths.contains(&path))
                    {
                        used.insert(path);
                    }
                }
                if let Some(paths) = references {
                    used.extend(paths);
                }
            }
        }
    }
    Ok(stored
        .iter()
        .filter(|path| path.starts_with("skillet/") && !used.contains(*path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn selected(module: &RequirementGroup, profile: &HostProfile) -> bool {
    module.required_by.iter().any(|service| {
        if service == "ui" {
            !profile.ui_services().is_empty()
        } else {
            profile.supports_service(service)
        }
    })
}

fn expand(path: &str, host: &str, environment: &str) -> String {
    path.replace("<host>", host)
        .replace("<environment>", environment)
}

/// Check required manual entries for the selected canonical profiles.
/// Generated values may be absent; their presence alone does not prove recovery.
pub fn check(
    profiles: &[HostProfile],
    policy: ProvisioningPolicy,
    lookup: &impl Fn(&str) -> Result<Option<String>, VaultError>,
) -> Result<CheckReport, CheckError> {
    let mut required = BTreeMap::new();
    for module in catalog()? {
        for profile in profiles.iter().filter(|profile| selected(&module, profile)) {
            let host = profile.id.as_str();
            let paths = module
                .template
                .as_deref()
                .map(|service| {
                    configuration_templates::secret_paths(service, host, policy.vault_name())
                })
                .transpose()?;
            let entries = module
                .entries
                .iter()
                .filter(|entry| entry.status == Status::Required);
            let documented = entries
                .map(|entry| (expand(&entry.path, host, policy.vault_name()), entry))
                .collect::<BTreeMap<_, _>>();
            let paths = paths.unwrap_or_else(|| documented.keys().cloned().collect());
            for path in paths {
                let entry = documented.get(&path).ok_or_else(|| {
                    CheckError::InvalidCatalog(format!(
                        "document template reference {path} in {}",
                        module.module
                    ))
                })?;
                required.entry(path).or_insert_with(|| {
                    (
                        module.module.clone(),
                        format!("{} {}", module.description, entry.help),
                    )
                });
            }
        }
    }
    let mut report = CheckReport {
        checked: required.len(),
        missing: Vec::new(),
    };
    for (path, (module, guide)) in required {
        let invalid = match lookup(&path) {
            Ok(Some(value)) if !value.is_empty() => continue,
            Ok(None) => false,
            Ok(Some(_)) | Err(VaultError::Invalid(_)) => true,
            Err(error) => return Err(error.into()),
        };
        report.missing.push(MissingEntry {
            module,
            path,
            guide,
            invalid,
        });
    }
    report.missing.sort_by(|a, b| {
        a.module
            .to_lowercase()
            .cmp(&b.module.to_lowercase())
            .then(a.path.cmp(&b.path))
    });
    Ok(report)
}

/// Render the same instructions used by the checker as the root checklist.
pub fn documentation() -> Result<String, CheckError> {
    let mut output = INTRO.to_string();
    for module in catalog()? {
        // Writing to String is infallible.
        let _ = write!(output, "\n## {}\n\n{}\n", module.module, module.description);
        if !module.entries.is_empty() {
            output.push_str(
                "\n| Entry | Status | Password value / how to obtain it |\n| --- | --- | --- |\n",
            );
            for entry in module.entries {
                let _ = writeln!(
                    output,
                    "| `{}` | {} | {} |",
                    entry.path,
                    entry.status.label(),
                    entry.help
                );
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
#[path = "secrets/tests.rs"]
mod tests;
