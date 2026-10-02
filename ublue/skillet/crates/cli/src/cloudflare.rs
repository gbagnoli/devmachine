use anyhow::{anyhow, Context, Result};
use cloudflare::framework::{
    auth::Credentials,
    client::{blocking_api::HttpApiClient, ClientConfig},
    endpoint::{spec::EndpointSpec, Method, RequestBody},
    response::ApiSuccess,
    Environment,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API_BASE: &str = "https://api.cloudflare.com/client/v4/";
const RECORD_TTL: u32 = 120;

#[cfg(test)]
#[path = "cloudflare_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RecordRef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IssuedToken {
    pub id: String,
    pub value: String,
    pub expires_on: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Zone {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct OwnedDns {
    pub marker: String,
    pub records: Vec<RecordRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DesiredRecord {
    pub name: String,
    pub record_type: String,
    pub content: String,
}

pub(crate) struct Cloudflare {
    base: String,
}

impl Cloudflare {
    pub(crate) fn new() -> Self {
        Self {
            base: API_BASE.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string() + "/",
        }
    }

    fn request(
        &self,
        token: &str,
        method: Method,
        path: String,
        query: Option<String>,
        body: Option<Value>,
    ) -> Result<ApiSuccess<JsonResult>> {
        let endpoint = ApiEndpoint {
            method,
            path,
            query,
            body: body
                .map(|value| serde_json::to_string(&value))
                .transpose()
                .context("encoding Cloudflare API request")?,
        };
        let environment = if self.base == API_BASE {
            Environment::Production
        } else {
            Environment::Custom(self.base.clone())
        };
        let client = HttpApiClient::new(
            Credentials::UserAuthToken {
                token: token.to_string(),
            },
            ClientConfig::default(),
            environment,
        )
        .context("building cloudflare-rs client")?;
        client
            .request(&endpoint)
            .map_err(|error| anyhow!("Cloudflare API request failed: {error}"))
    }

    fn result(
        &self,
        token: &str,
        method: Method,
        path: String,
        query: Option<String>,
        body: Option<Value>,
    ) -> Result<Value> {
        Ok(self.request(token, method, path, query, body)?.result.0)
    }

    pub(crate) fn zone(&self, token: &str, zone_id: &str) -> Result<Zone> {
        validate_zone_id(zone_id)?;
        let value = self.result(token, Method::GET, format!("zones/{zone_id}"), None, None)?;
        serde_json::from_value(value).context("decoding Cloudflare zone")
    }

    fn permission_groups(&self, token: &str) -> Result<(String, String)> {
        let value = self.result(
            token,
            Method::GET,
            "user/tokens/permission_groups".to_string(),
            Some("per_page=500".to_string()),
            None,
        )?;
        let groups = value
            .as_array()
            .ok_or_else(|| anyhow!("Cloudflare permission group response is malformed"))?;
        let mut zone_read = None;
        let mut dns_write = None;
        for group in groups {
            if group.get("is_selectable").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            let Some(id) = group.get("id").and_then(Value::as_str) else {
                continue;
            };
            let name = group
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let scopes = group
                .get("scopes")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            if name == "Zone Read" && scopes.contains("com.cloudflare.api.account.zone") {
                zone_read = Some(id.to_string());
            }
            if matches!(name, "DNS Write" | "DNS Edit")
                && scopes.contains("com.cloudflare.api.account.zone")
            {
                dns_write = Some(id.to_string());
            }
        }
        match (zone_read, dns_write) {
            (Some(read), Some(write)) => Ok((read, write)),
            _ => Err(anyhow!("Cloudflare did not expose selectable Zone Read and DNS Write permission groups; the token creator needs User > API Tokens > Read and Write")),
        }
    }

    pub(crate) fn create_zone_token(
        &self,
        creator_token: &str,
        zone_id: &str,
        name: &str,
        lifetime: Option<Duration>,
    ) -> Result<IssuedToken> {
        validate_zone_id(zone_id)?;
        let (zone_read, dns_write) = self.permission_groups(creator_token)?;
        let mut payload = json!({
            "name": name,
            "policies": [{
                "effect": "allow",
                "permission_groups": [{"id": zone_read}, {"id": dns_write}],
                "resources": {format!("com.cloudflare.api.account.zone.{zone_id}"): "*"}
            }]
        });
        if let Some(lifetime) = lifetime {
            payload["expires_on"] = json!(expiry_rfc3339(lifetime)?);
        }
        let value = self.result(
            creator_token,
            Method::POST,
            "user/tokens".to_string(),
            None,
            Some(payload),
        )?;
        serde_json::from_value(value).context("decoding newly issued Cloudflare token")
    }

    pub(crate) fn token_ids_by_name(&self, creator_token: &str, name: &str) -> Result<Vec<String>> {
        let mut matches = Vec::new();
        for page in 1..=100_u32 {
            let response = self.request(
                creator_token,
                Method::GET,
                "user/tokens".to_string(),
                Some(format!("page={page}&per_page=100")),
                None,
            )?;
            let values = response
                .result
                .0
                .as_array()
                .ok_or_else(|| anyhow!("Cloudflare token list response is malformed"))?;
            matches.extend(values.iter().filter_map(|token| {
                (token.get("name").and_then(Value::as_str) == Some(name))
                    .then(|| token.get("id").and_then(Value::as_str))
                    .flatten()
                    .filter(|id| validate_token_id(id).is_ok())
                    .map(ToOwned::to_owned)
            }));
            let total_pages = response
                .result_info
                .as_ref()
                .and_then(|info| info.get("total_pages"))
                .and_then(Value::as_u64)
                .unwrap_or(u64::from(page));
            if u64::from(page) >= total_pages {
                break;
            }
        }
        Ok(matches)
    }

    pub(crate) fn revoke_token(&self, creator_token: &str, token_id: &str) -> Result<()> {
        validate_token_id(token_id)?;
        self.result(
            creator_token,
            Method::DELETE,
            format!("user/tokens/{token_id}"),
            None,
            None,
        )?;
        Ok(())
    }

    fn list_records(&self, token: &str, zone_id: &str) -> Result<Vec<RecordRef>> {
        validate_zone_id(zone_id)?;
        let mut records = Vec::new();
        for page in 1..=100_u32 {
            let response = self.request(
                token,
                Method::GET,
                format!("zones/{zone_id}/dns_records"),
                Some(format!("page={page}&per_page=100")),
                None,
            )?;
            let batch: Vec<RecordRef> = serde_json::from_value(response.result.0)?;
            let count = batch.len();
            records.extend(batch);
            let total_pages = response
                .result_info
                .as_ref()
                .and_then(|info| info.get("total_pages"))
                .and_then(Value::as_u64)
                .unwrap_or(u64::from(page));
            if u64::from(page) >= total_pages || count == 0 {
                break;
            }
        }
        Ok(records)
    }

    fn create_record(
        &self,
        token: &str,
        zone_id: &str,
        name: &str,
        record_type: &str,
        content: &str,
        marker: &str,
    ) -> Result<RecordRef> {
        let value = self.result(
            token,
            Method::POST,
            format!("zones/{zone_id}/dns_records"),
            None,
            Some(json!({
                "type": record_type,
                "name": name,
                "content": content,
                "ttl": RECORD_TTL,
                "proxied": false,
                "comment": marker
            })),
        )?;
        serde_json::from_value(value).context("decoding created DNS record")
    }

    fn update_record(
        &self,
        token: &str,
        zone_id: &str,
        existing: &RecordRef,
        content: &str,
        marker: &str,
    ) -> Result<RecordRef> {
        validate_record_id(&existing.id)?;
        let value = self.result(
            token,
            Method::PATCH,
            format!("zones/{zone_id}/dns_records/{}", existing.id),
            None,
            Some(json!({
                "type": existing.record_type,
                "name": existing.name,
                "content": content,
                "ttl": RECORD_TTL,
                "proxied": false,
                "comment": marker
            })),
        )?;
        serde_json::from_value(value).context("decoding updated DNS record")
    }

    fn delete_record(&self, token: &str, zone_id: &str, id: &str) -> Result<()> {
        validate_record_id(id)?;
        self.result(
            token,
            Method::DELETE,
            format!("zones/{zone_id}/dns_records/{id}"),
            None,
            None,
        )?;
        Ok(())
    }

    pub(crate) fn reconcile_dns(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
        desired: &[DesiredRecord],
    ) -> Result<OwnedDns> {
        validate_zone_id(zone_id)?;
        validate_fqdn(ui_domain)?;
        if marker.is_empty() || marker.len() > 255 {
            return Err(anyhow!("invalid Skillet DNS ownership marker"));
        }
        let mut wanted: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for record in desired {
            validate_fqdn(&record.name)?;
            validate_record_content(&record.record_type, &record.content)?;
            wanted
                .entry((record.name.to_ascii_lowercase(), record.record_type.clone()))
                .or_default()
                .insert(record.content.clone());
        }
        let mut records = self.list_records(token, zone_id)?;
        let desired_names = desired
            .iter()
            .map(|record| record.name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        for record in records
            .iter()
            .filter(|record| desired_names.contains(&record.name.to_ascii_lowercase()))
        {
            if record.comment.as_deref() != Some(marker) {
                return Err(anyhow!(
                    "DNS name {} has a record not owned by this Skillet deployment",
                    record.name
                ));
            }
        }

        let mut retained = Vec::new();
        for ((name, record_type), contents) in &wanted {
            let mut existing = records
                .iter_mut()
                .filter(|record| {
                    record.name.eq_ignore_ascii_case(name)
                        && record.record_type == *record_type
                        && record.comment.as_deref() == Some(marker)
                })
                .collect::<Vec<_>>();
            for content in contents {
                if let Some(index) = existing
                    .iter()
                    .position(|record| record.content == *content)
                {
                    retained.push(existing.remove(index).clone());
                } else if let Some(record) = existing.pop() {
                    let updated = self.update_record(token, zone_id, record, content, marker)?;
                    retained.push(updated);
                } else {
                    retained.push(self.create_record(
                        token,
                        zone_id,
                        name,
                        record_type,
                        content,
                        marker,
                    )?);
                }
            }
            for duplicate in existing {
                self.delete_record(token, zone_id, &duplicate.id)?;
            }
        }
        let retained_ids = retained
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        for stale in records
            .drain(..)
            .filter(|record| record.comment.as_deref() == Some(marker))
        {
            if stale.name != ui_domain && !stale.name.ends_with(&format!(".{ui_domain}")) {
                return Err(anyhow!("Skillet ownership marker is present outside the configured UI domain; refusing cleanup"));
            }
            if !retained_ids.contains(stale.id.as_str()) {
                self.delete_record(token, zone_id, &stale.id)?;
            }
        }
        retained.sort_by(|left, right| {
            (&left.name, &left.record_type, &left.content).cmp(&(
                &right.name,
                &right.record_type,
                &right.content,
            ))
        });
        Ok(OwnedDns {
            marker: marker.to_string(),
            records: retained,
        })
    }

    pub(crate) fn remove_dns_marker(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
    ) -> Result<()> {
        validate_zone_id(zone_id)?;
        validate_fqdn(ui_domain)?;
        for record in self.list_records(token, zone_id)? {
            if record.comment.as_deref() != Some(marker) {
                continue;
            }
            if !matches!(record.record_type.as_str(), "A" | "AAAA" | "CNAME")
                || !(record.name == ui_domain || record.name.ends_with(&format!(".{ui_domain}")))
            {
                return Err(anyhow!("Skillet ownership marker appears on an unexpected DNS record; refusing cleanup"));
            }
            self.delete_record(token, zone_id, &record.id)?;
        }
        Ok(())
    }
}

pub(crate) fn desired_records(
    machine_name: &str,
    addresses: &BTreeSet<String>,
    sites: &skillet_caddy::CaddySites,
) -> Result<Vec<DesiredRecord>> {
    validate_fqdn(machine_name)?;
    let mut result = Vec::new();
    let mut has_v4 = false;
    let mut has_v6 = false;
    for address in addresses {
        let parsed = address
            .parse::<IpAddr>()
            .context("invalid Tailscale IP address")?;
        has_v4 |= parsed.is_ipv4();
        has_v6 |= parsed.is_ipv6();
        result.push(DesiredRecord {
            name: machine_name.to_string(),
            record_type: if parsed.is_ipv4() { "A" } else { "AAAA" }.to_string(),
            content: parsed.to_string(),
        });
    }
    if !has_v4 || !has_v6 {
        return Err(anyhow!(
            "Tailscale device must have both IPv4 and IPv6 addresses before publishing DNS"
        ));
    }
    for service in &sites.services {
        result.push(DesiredRecord {
            name: service.hostname.clone(),
            record_type: "CNAME".to_string(),
            content: machine_name.to_string(),
        });
        for alias in &service.aliases {
            result.push(DesiredRecord {
                name: alias.clone(),
                record_type: "CNAME".to_string(),
                content: service.hostname.clone(),
            });
        }
    }
    Ok(result)
}

pub(crate) fn validate_zone_id(id: &str) -> Result<()> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "Cloudflare Zone ID must be 32 hexadecimal characters"
        ));
    }
    Ok(())
}

pub(crate) fn validate_token_id(id: &str) -> Result<()> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("Cloudflare token ID is malformed"));
    }
    Ok(())
}

fn validate_record_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("Cloudflare DNS record ID is malformed"));
    }
    Ok(())
}

fn validate_record_content(record_type: &str, content: &str) -> Result<()> {
    match record_type {
        "A" | "AAAA" => {
            let address = content
                .parse::<IpAddr>()
                .context("invalid IP in DNS plan")?;
            if (record_type == "A") != address.is_ipv4() {
                return Err(anyhow!("DNS record address family does not match type"));
            }
        }
        "CNAME" => validate_fqdn(content)?,
        _ => return Err(anyhow!("unsupported managed DNS record type")),
    }
    Ok(())
}

fn validate_fqdn(name: &str) -> Result<()> {
    skillet_caddy::validate_domain(name).map_err(|error| anyhow!("{error}"))
}

fn expiry_rfc3339(lifetime: Duration) -> Result<String> {
    let seconds = SystemTime::now()
        .checked_add(lifetime)
        .ok_or_else(|| anyhow!("Cloudflare token expiry overflow"))?
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(i64::try_from(days)?);
    if !(1..=9999).contains(&year) {
        return Err(anyhow!("Cloudflare token expiry is outside RFC3339 range"));
    }
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    ))
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[derive(Clone, Debug)]
struct JsonResult(Value);

impl<'de> Deserialize<'de> for JsonResult {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self)
    }
}

impl cloudflare::framework::response::ApiResult for JsonResult {}

#[derive(Debug)]
struct ApiEndpoint {
    method: Method,
    path: String,
    query: Option<String>,
    body: Option<String>,
}

impl EndpointSpec for ApiEndpoint {
    type JsonResponse = JsonResult;
    type ResponseType = ApiSuccess<Self::JsonResponse>;

    fn method(&self) -> Method {
        self.method.clone()
    }

    fn path(&self) -> String {
        self.path.clone()
    }

    fn query(&self) -> Option<String> {
        self.query.clone()
    }

    fn body(&self) -> Option<RequestBody<'_>> {
        self.body.clone().map(RequestBody::Json)
    }
}
