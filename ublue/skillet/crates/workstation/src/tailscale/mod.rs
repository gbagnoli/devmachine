use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    error::Error as StdError,
    fmt::Write as _,
    time::{Duration, Instant},
};
use thiserror::Error;

const API_BASE: &str = "https://api.tailscale.com/api/v2";
pub const PROVISIONER_TAG: &str = "tag:skillet-provisioner";
pub const SERVER_TAG: &str = "tag:skillet-server";
pub const SMOKE_TAG: &str = "tag:skillet-smoke";

#[derive(Debug, Error)]
pub enum TailscaleError {
    #[error("{context}: {source}")]
    Source {
        context: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, TailscaleError>;

fn source<E>(context: impl Into<String>, error: E) -> TailscaleError
where
    E: StdError + Send + Sync + 'static,
{
    TailscaleError::Source {
        context: context.into(),
        source: Box::new(error),
    }
}

trait ResultContext<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;
}

impl<T, E> ResultContext<T> for std::result::Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|error| source(context, error))
    }
}

impl From<serde_json::Error> for TailscaleError {
    fn from(error: serde_json::Error) -> Self {
        source("Tailscale JSON", error)
    }
}

pub struct OAuthCredentials {
    client_id: String,
    client_secret: String,
    api: ApiClient,
}

struct ApiClient {
    base_url: String,
    http: Client,
}

impl ApiClient {
    fn new(base_url: &str) -> Result<Self> {
        let base_url = base_url.trim_end_matches('/');
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(TailscaleError::Invalid(
                "invalid Tailscale API base URL".into(),
            ));
        }
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("building Tailscale HTTP client")?;
        Ok(Self {
            base_url: base_url.to_string(),
            http,
        })
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        bearer: Option<&str>,
        body: Option<(&str, &str)>,
    ) -> Result<Value> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base_url));
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        if let Some((content_type, payload)) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(payload.to_string());
        }
        let response = request.send().context("sending Tailscale API request")?;
        let status = response.status();
        let payload = response.text().context("reading Tailscale API response")?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&payload)
                .ok()
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            return Err(match message {
                Some(message) => TailscaleError::Invalid(format!(
                    "Tailscale API returned HTTP {}: {message}",
                    status.as_u16()
                )),
                None => TailscaleError::Invalid(format!(
                    "Tailscale API request failed with HTTP {}",
                    status.as_u16()
                )),
            });
        }
        if payload.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&payload).context("decoding Tailscale API response")
    }
}

impl OAuthCredentials {
    pub fn new(client_id: String, client_secret: String) -> Result<Self> {
        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return Err(TailscaleError::Invalid(
                "Tailscale OAuth credentials are empty".into(),
            ));
        }
        Ok(Self {
            client_id,
            client_secret,
            api: ApiClient::new(API_BASE)?,
        })
    }

    #[cfg(test)]
    fn with_api_base(client_id: String, client_secret: String, base_url: &str) -> Result<Self> {
        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return Err(TailscaleError::Invalid(
                "Tailscale OAuth credentials are empty".into(),
            ));
        }
        Ok(Self {
            client_id,
            client_secret,
            api: ApiClient::new(base_url)?,
        })
    }

    fn access_token(&self, scope: &str) -> Result<String> {
        let form = [
            ("grant_type", "client_credentials"),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("scope", scope),
            ("tags", PROVISIONER_TAG),
        ]
        .into_iter()
        .map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
        let response = self.api.request(
            reqwest::Method::POST,
            "/oauth/token",
            None,
            Some(("application/x-www-form-urlencoded", &form)),
        )?;
        response
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                TailscaleError::Invalid(
                    "Tailscale OAuth response did not contain an access token".into(),
                )
            })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceRecord {
    pub id: String,
    pub hostname: String,
    pub addresses: BTreeSet<String>,
}

/// Parse the local `tailscale status --json` response used by the guest
/// enrollment probe. A non-running backend has no usable addresses yet.
pub fn status_addresses(output: &[u8]) -> Result<BTreeSet<String>> {
    let status: Value = serde_json::from_slice(output)
        .map_err(|error| source("decoding Tailscale status", error))?;
    if status.get("BackendState").and_then(Value::as_str) != Some("Running") {
        return Ok(BTreeSet::new());
    }
    let addresses = status
        .get("Self")
        .and_then(|value| value.get("TailscaleIPs"))
        .and_then(Value::as_array)
        .ok_or_else(|| TailscaleError::Invalid("Tailscale status has no self addresses".into()))?
        .iter()
        .filter_map(Value::as_str)
        .filter(|address| address.parse::<std::net::IpAddr>().is_ok())
        .map(ToOwned::to_owned)
        .collect();
    Ok(addresses)
}

/// Poll a guest status probe until it reports at least one address or times
/// out. Probe errors and empty status are transient during enrollment.
pub fn wait_for_addresses<E>(
    mut probe: impl FnMut() -> std::result::Result<BTreeSet<String>, E>,
    timeout: Duration,
    interval: Duration,
) -> Result<BTreeSet<String>> {
    let started = Instant::now();
    loop {
        if let Ok(addresses) = probe() {
            if !addresses.is_empty() {
                return Ok(addresses);
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            break;
        }
        std::thread::sleep(interval.min(timeout.saturating_sub(elapsed)));
    }
    Err(TailscaleError::Invalid(
        "Tailscale did not connect on the VM before the enrollment timeout".into(),
    ))
}

pub struct AuthKey {
    pub key: String,
}

pub fn create_auth_key(
    credentials: &OAuthCredentials,
    target_tag: &str,
    description: &str,
) -> Result<AuthKey> {
    if !matches!(target_tag, SERVER_TAG | SMOKE_TAG) {
        return Err(TailscaleError::Invalid(
            "unsupported Tailscale device tag".into(),
        ));
    }
    let token = credentials.access_token("auth_keys")?;
    let payload = json!({
        "capabilities": {"devices": {"create": {
            "reusable": false,
            "ephemeral": false,
            "preauthorized": true,
            "tags": [target_tag]
        }}},
        "expirySeconds": 3600,
        "description": description
    });
    let body = serde_json::to_string(&payload).context("encoding Tailscale auth key request")?;
    let response = credentials.api.request(
        reqwest::Method::POST,
        "/tailnet/-/keys",
        Some(&token),
        Some(("application/json", &body)),
    )?;
    let key = response
        .get("key")
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            TailscaleError::Invalid("Tailscale auth key response did not contain a key".into())
        })?
        .to_owned();
    Ok(AuthKey { key })
}

pub fn find_device(
    credentials: &OAuthCredentials,
    expected_hostname: &str,
    expected_tag: &str,
    addresses: &BTreeSet<String>,
) -> Result<DeviceRecord> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || addresses.is_empty() {
        return Err(TailscaleError::Invalid(
            "invalid Tailscale device lookup constraints".into(),
        ));
    }
    let token = credentials.access_token("devices:core")?;
    let response = credentials.api.request(
        reqwest::Method::GET,
        "/tailnet/-/devices",
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TailscaleError::Invalid("Tailscale device list response is malformed".into())
        })?;
    let matches = devices
        .iter()
        .filter_map(|device| device_record(device, expected_tag))
        .filter(|device| {
            device.hostname == expected_hostname
                && device
                    .addresses
                    .iter()
                    .any(|address| addresses.contains(address))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [device] => Ok(device.clone()),
        [] => Err(TailscaleError::Invalid(
            "Tailscale has not registered the expected device yet".into(),
        )),
        _ => Err(TailscaleError::Invalid(
            "multiple Tailscale devices match this VM; refusing ambiguity".into(),
        )),
    }
}

pub fn find_device_by_hostname(
    credentials: &OAuthCredentials,
    expected_hostname: &str,
    expected_tag: &str,
) -> Result<DeviceRecord> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || expected_hostname.is_empty() {
        return Err(TailscaleError::Invalid(
            "invalid Tailscale device lookup constraints".into(),
        ));
    }
    let token = credentials.access_token("devices:core")?;
    let response = credentials.api.request(
        reqwest::Method::GET,
        "/tailnet/-/devices",
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TailscaleError::Invalid("Tailscale device list response is malformed".into())
        })?;
    let matches = devices
        .iter()
        .filter_map(|device| device_record(device, expected_tag))
        .filter(|device| device.hostname == expected_hostname)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [device] if !device.addresses.is_empty() => Ok(device.clone()),
        [] => Err(TailscaleError::Invalid(format!(
            "Tailscale has no device named {expected_hostname} with tag {expected_tag}"
        ))),
        [_] => Err(TailscaleError::Invalid("Tailscale device has no registered addresses".into())),
        _ => Err(TailscaleError::Invalid(format!(
            "multiple tagged Tailscale devices have hostname {expected_hostname}; refusing ambiguity"
        ))),
    }
}

pub fn remove_device_for_hostname(
    credentials: &OAuthCredentials,
    hostname: &str,
    expected_tag: &str,
    expected: Option<&DeviceRecord>,
) -> Result<Option<DeviceRecord>> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || hostname.is_empty() {
        return Err(TailscaleError::Invalid(
            "invalid Tailscale device removal constraints".into(),
        ));
    }
    let token = credentials.access_token("devices:core")?;
    let response = credentials.api.request(
        reqwest::Method::GET,
        "/tailnet/-/devices",
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TailscaleError::Invalid("Tailscale device list response is malformed".into())
        })?;
    let matches = devices
        .iter()
        .filter_map(|device| device_record(device, expected_tag))
        .filter(|device| device.hostname == hostname)
        .collect::<Vec<_>>();
    let actual = match matches.as_slice() {
        [] => return Ok(None),
        [actual] => actual,
        _ => {
            return Err(TailscaleError::Invalid(format!(
                "multiple tagged Tailscale devices have hostname {hostname}; refusing removal"
            )))
        }
    };
    if let Some(expected) = expected {
        if actual.id != expected.id
            || !actual
                .addresses
                .iter()
                .any(|address| expected.addresses.contains(address))
        {
            return Err(TailscaleError::Invalid(
                "recorded Tailscale ID no longer matches this VM; refusing removal".into(),
            ));
        }
    }
    credentials.api.request(
        reqwest::Method::DELETE,
        &format!("/device/{}", path_segment(&actual.id)?),
        Some(&token),
        None,
    )?;
    Ok(Some(actual.clone()))
}

fn device_record(value: &Value, expected_tag: &str) -> Option<DeviceRecord> {
    let tags = value.get("tags")?.as_array()?;
    if !tags.iter().any(|tag| tag.as_str() == Some(expected_tag)) {
        return None;
    }
    let id = value.get("id")?.as_str()?.to_owned();
    let hostname = value.get("hostname")?.as_str()?.to_owned();
    let addresses = value
        .get("addresses")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect();
    Some(DeviceRecord {
        id,
        hostname,
        addresses,
    })
}

fn form_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn path_segment(value: &str) -> Result<String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(TailscaleError::Invalid(
            "invalid Tailscale device ID".into(),
        ));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
