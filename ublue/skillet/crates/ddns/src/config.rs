//! Private input and the pinned updater's JSON contract.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid DDNS JSON or schema")]
    Json,
    #[error("unsupported DDNS config version")]
    Version,
    #[error("DDNS requires at least one valid, unique relative record name")]
    Records,
    #[error("DDNS record conflicts with a private UI record")]
    Overlap,
    #[error("invalid DDNS zone or credential")]
    Credential,
    #[error("unsupported DDNS address, TTL, or purge policy")]
    Policy,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub name: String,
    pub proxied: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateConfig {
    pub version: u32,
    pub records: Vec<Record>,
    /// Required explicit approval before adopting an existing public A record.
    #[serde(default)]
    pub takeover_existing: bool,
}

impl PrivateConfig {
    pub fn parse(json: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(json).map_err(|_| ConfigError::Json)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.version != 1 {
            return Err(ConfigError::Version);
        }
        let mut names = BTreeSet::new();
        if self.records.is_empty()
            || self
                .records
                .iter()
                .any(|record| !valid_dns_name(&record.name) || !names.insert(record.name.as_str()))
        {
            return Err(ConfigError::Records);
        }
        Ok(())
    }

    /// Resolve below the zone itself, independently of the private UI prefix.
    pub fn validate_zone(&self, zone: &str, ui_records: &[String]) -> Result<(), ConfigError> {
        self.validate()?;
        if !valid_dns_name(zone) {
            return Err(ConfigError::Credential);
        }
        for record in &self.records {
            let name = format!("{}.{zone}", record.name);
            if name.len() > 253 {
                return Err(ConfigError::Records);
            }
            if ui_records.iter().any(|ui| ui.eq_ignore_ascii_case(&name)) {
                return Err(ConfigError::Overlap);
            }
        }
        Ok(())
    }

    pub fn payload(&self, zone_id: &str, token: &str) -> Result<Payload, ConfigError> {
        self.validate()?;
        let payload = Payload {
            cloudflare: vec![ZoneConfig {
                authentication: Authentication {
                    api_token: token.to_string(),
                },
                zone_id: zone_id.to_string(),
                subdomains: self.records.clone(),
            }],
            a: true,
            aaaa: false,
            purge_unknown_records: false,
            ttl: 300,
            record_comment: None,
        };
        payload.validate()?;
        Ok(payload)
    }
}

// Deliberately no Debug: payloads contain credentials and private DNS names.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    cloudflare: Vec<ZoneConfig>,
    a: bool,
    aaaa: bool,
    #[serde(rename = "purgeUnknownRecords")]
    purge_unknown_records: bool,
    ttl: u32,
    #[serde(
        rename = "recordComment",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    record_comment: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ZoneConfig {
    authentication: Authentication,
    zone_id: String,
    subdomains: Vec<Record>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Authentication {
    api_token: String,
}

impl Payload {
    /// Distinct from UI ownership markers; the updater preserves this comment.
    pub fn with_record_comment(mut self, comment: &str) -> Result<Self, ConfigError> {
        if !comment.starts_with("skillet-ddns:")
            || comment.len() > 200
            || comment.chars().any(char::is_control)
        {
            return Err(ConfigError::Credential);
        }
        self.record_comment = Some(comment.to_string());
        Ok(self)
    }
    pub fn parse(json: &str) -> Result<Self, ConfigError> {
        let payload: Self = serde_json::from_str(json).map_err(|_| ConfigError::Json)?;
        payload.validate()?;
        Ok(payload)
    }

    pub fn render(&self) -> Result<String, ConfigError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| ConfigError::Json)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.record_comment.as_ref().is_some_and(|comment| {
            !comment.starts_with("skillet-ddns:")
                || comment.len() > 200
                || comment.chars().any(char::is_control)
        }) {
            return Err(ConfigError::Credential);
        }
        if !self.a || self.aaaa || self.purge_unknown_records || self.ttl != 300 {
            return Err(ConfigError::Policy);
        }
        let [zone] = self.cloudflare.as_slice() else {
            return Err(ConfigError::Credential);
        };
        if zone.zone_id.len() != 32
            || !zone.zone_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || zone.authentication.api_token.is_empty()
            || zone.authentication.api_token.trim() != zone.authentication.api_token
            || zone.authentication.api_token.chars().any(char::is_control)
        {
            return Err(ConfigError::Credential);
        }
        PrivateConfig {
            version: 1,
            records: zone.subdomains.clone(),
            takeover_existing: false,
        }
        .validate()
    }
}

fn valid_dns_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
