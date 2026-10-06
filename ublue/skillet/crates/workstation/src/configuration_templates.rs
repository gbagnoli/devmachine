//! Workstation-side configuration templates with explicit `KeePassXC` references.
use crate::vault::{Vault, VaultError};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

const TEMPLATE_CATALOG: &str = include_str!("configuration-templates.json");

#[derive(Debug, Error)]
pub enum ConfigurationTemplateError {
    #[error("configuration template catalog is invalid: {0}")]
    Catalog(#[source] serde_json::Error),
    #[error("could not serialize rendered configuration")]
    Serialization(#[source] serde_json::Error),
    #[error("unsupported configuration template catalog version")]
    Version,
    #[error("no configuration template registered for service {service} and host {host}")]
    MissingTemplate { service: String, host: String },
    #[error("host or environment is not a valid template path component")]
    InvalidContext,
    #[error("invalid secret reference in configuration template")]
    InvalidReference,
    #[error("KeePassXC entry is missing: {0}")]
    MissingSecret(String),
    #[error(transparent)]
    Vault(#[from] VaultError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateCatalog {
    version: u32,
    services: BTreeMap<String, ServiceTemplates>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceTemplates {
    default: Value,
    #[serde(default)]
    hosts: BTreeMap<String, Value>,
}

/// Render the selected service template and resolve typed references through
/// `KeePassXC`. A host-specific template replaces the complete shared template.
/// Secret values are inserted as JSON strings and are never interpolated into
/// surrounding text.
pub fn render(
    service: &str,
    host: &str,
    environment: &str,
    vault: &Vault,
) -> Result<String, ConfigurationTemplateError> {
    render_catalog(TEMPLATE_CATALOG, service, host, environment, &|path| {
        vault.get(path)
    })
}

fn render_catalog(
    catalog: &str,
    service: &str,
    host: &str,
    environment: &str,
    lookup: &impl Fn(&str) -> Result<Option<String>, VaultError>,
) -> Result<String, ConfigurationTemplateError> {
    if !valid_component(host) || !valid_component(environment) {
        return Err(ConfigurationTemplateError::InvalidContext);
    }
    let catalog: TemplateCatalog =
        serde_json::from_str(catalog).map_err(ConfigurationTemplateError::Catalog)?;
    if catalog.version != 1 {
        return Err(ConfigurationTemplateError::Version);
    }
    let service_templates = catalog.services.get(service).ok_or_else(|| {
        ConfigurationTemplateError::MissingTemplate {
            service: service.to_string(),
            host: host.to_string(),
        }
    })?;
    let mut value = service_templates
        .hosts
        .get(host)
        .unwrap_or(&service_templates.default)
        .clone();
    resolve_value(&mut value, host, environment, lookup)?;
    serde_json::to_string(&value).map_err(ConfigurationTemplateError::Serialization)
}

fn resolve_value(
    value: &mut Value,
    host: &str,
    environment: &str,
    lookup: &impl Fn(&str) -> Result<Option<String>, VaultError>,
) -> Result<(), ConfigurationTemplateError> {
    match value {
        Value::Object(fields) => {
            if let Some(reference) = fields.get("$secret") {
                if fields.len() != 1 {
                    return Err(ConfigurationTemplateError::InvalidReference);
                }
                let Value::String(reference) = reference else {
                    return Err(ConfigurationTemplateError::InvalidReference);
                };
                let path = resolve_entry_path(reference, host, environment)?;
                let secret = lookup(&path)?
                    .ok_or_else(|| ConfigurationTemplateError::MissingSecret(path.clone()))?;
                *value = Value::String(secret);
            } else {
                for child in fields.values_mut() {
                    resolve_value(child, host, environment, lookup)?;
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                resolve_value(child, host, environment, lookup)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn resolve_entry_path(
    reference: &str,
    host: &str,
    environment: &str,
) -> Result<String, ConfigurationTemplateError> {
    let path = reference
        .replace("{host}", host)
        .replace("{environment}", environment);
    if !path.starts_with("skillet/")
        || path.contains('{')
        || path.contains('}')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ConfigurationTemplateError::InvalidReference);
    }
    Ok(path)
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
#[path = "configuration_templates/tests.rs"]
mod tests;
