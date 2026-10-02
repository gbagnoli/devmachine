use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    io::Write as _,
    process::{Command, Stdio},
};

const API_BASE: &str = "https://api.tailscale.com/api/v2";
pub(crate) const PROVISIONER_TAG: &str = "tag:skillet-provisioner";
pub(crate) const SERVER_TAG: &str = "tag:skillet-server";
pub(crate) const SMOKE_TAG: &str = "tag:skillet-smoke";

pub(crate) struct OAuthCredentials {
    client_id: String,
    client_secret: String,
}

impl OAuthCredentials {
    pub(crate) fn new(client_id: String, client_secret: String) -> Result<Self> {
        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return Err(anyhow!("Tailscale OAuth credentials are empty"));
        }
        Ok(Self {
            client_id,
            client_secret,
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
        let response = request(
            "POST",
            &format!("{API_BASE}/oauth/token"),
            None,
            Some(("application/x-www-form-urlencoded", &form)),
        )?;
        response
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("Tailscale OAuth response did not contain an access token"))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct DeviceRecord {
    pub(crate) id: String,
    pub(crate) hostname: String,
    pub(crate) addresses: BTreeSet<String>,
}

pub(crate) struct AuthKey {
    pub(crate) key: String,
}

pub(crate) fn create_auth_key(
    credentials: &OAuthCredentials,
    target_tag: &str,
    description: &str,
) -> Result<AuthKey> {
    if !matches!(target_tag, SERVER_TAG | SMOKE_TAG) {
        return Err(anyhow!("unsupported Tailscale device tag"));
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
    let response = request(
        "POST",
        &format!("{API_BASE}/tailnet/-/keys"),
        Some(&token),
        Some(("application/json", &body)),
    )?;
    let key = response
        .get("key")
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
        .ok_or_else(|| anyhow!("Tailscale auth key response did not contain a key"))?
        .to_owned();
    Ok(AuthKey { key })
}

pub(crate) fn find_device(
    credentials: &OAuthCredentials,
    expected_hostname: &str,
    expected_tag: &str,
    addresses: &BTreeSet<String>,
) -> Result<DeviceRecord> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || addresses.is_empty() {
        return Err(anyhow!("invalid Tailscale device lookup constraints"));
    }
    let token = credentials.access_token("devices:core")?;
    let response = request(
        "GET",
        &format!("{API_BASE}/tailnet/-/devices"),
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Tailscale device list response is malformed"))?;
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
        [] => Err(anyhow!(
            "Tailscale has not registered the expected device yet"
        )),
        _ => Err(anyhow!(
            "multiple Tailscale devices match this VM; refusing ambiguity"
        )),
    }
}

pub(crate) fn find_device_by_hostname(
    credentials: &OAuthCredentials,
    expected_hostname: &str,
    expected_tag: &str,
) -> Result<DeviceRecord> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || expected_hostname.is_empty() {
        return Err(anyhow!("invalid Tailscale device lookup constraints"));
    }
    let token = credentials.access_token("devices:core")?;
    let response = request(
        "GET",
        &format!("{API_BASE}/tailnet/-/devices"),
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Tailscale device list response is malformed"))?;
    let matches = devices
        .iter()
        .filter_map(|device| device_record(device, expected_tag))
        .filter(|device| device.hostname == expected_hostname)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [device] if !device.addresses.is_empty() => Ok(device.clone()),
        [] => Err(anyhow!(
            "Tailscale has no device named {expected_hostname} with tag {expected_tag}"
        )),
        [_] => Err(anyhow!("Tailscale device has no registered addresses")),
        _ => Err(anyhow!(
            "multiple tagged Tailscale devices have hostname {expected_hostname}; refusing ambiguity"
        )),
    }
}

pub(crate) fn remove_device_for_hostname(
    credentials: &OAuthCredentials,
    hostname: &str,
    expected_tag: &str,
    expected: Option<&DeviceRecord>,
) -> Result<Option<DeviceRecord>> {
    if !matches!(expected_tag, SERVER_TAG | SMOKE_TAG) || hostname.is_empty() {
        return Err(anyhow!("invalid Tailscale device removal constraints"));
    }
    let token = credentials.access_token("devices:core")?;
    let response = request(
        "GET",
        &format!("{API_BASE}/tailnet/-/devices"),
        Some(&token),
        None,
    )?;
    let devices = response
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Tailscale device list response is malformed"))?;
    let matches = devices
        .iter()
        .filter_map(|device| device_record(device, expected_tag))
        .filter(|device| device.hostname == hostname)
        .collect::<Vec<_>>();
    let actual = match matches.as_slice() {
        [] => return Ok(None),
        [actual] => actual,
        _ => {
            return Err(anyhow!(
                "multiple tagged Tailscale devices have hostname {hostname}; refusing removal"
            ))
        }
    };
    if let Some(expected) = expected {
        if actual.id != expected.id
            || !actual
                .addresses
                .iter()
                .any(|address| expected.addresses.contains(address))
        {
            return Err(anyhow!(
                "recorded Tailscale ID no longer matches this VM; refusing removal"
            ));
        }
    }
    request(
        "DELETE",
        &format!("{API_BASE}/device/{}", path_segment(&actual.id)?),
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

fn request(
    method: &str,
    url: &str,
    bearer: Option<&str>,
    body: Option<(&str, &str)>,
) -> Result<Value> {
    let mut config = format!(
        "url = {}\nrequest = {}\n",
        curl_quote(url),
        curl_quote(method)
    );
    if let Some(token) = bearer {
        let _ = writeln!(
            config,
            "header = {}",
            curl_quote(&format!("Authorization: Bearer {token}"))
        );
    }
    if let Some((content_type, payload)) = body {
        let _ = write!(
            config,
            "header = {}\ndata = {}\n",
            curl_quote(&format!("Content-Type: {content_type}")),
            curl_quote(payload)
        );
    }
    let mut child = Command::new("curl")
        .args([
            "--config",
            "-",
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--write-out",
            "\n%{http_code}",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting curl for Tailscale API; install curl on the workstation")?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("curl stdin unavailable"))?
        .write_all(config.as_bytes())
        .context("sending private Tailscale API request to curl")?;
    let output = child
        .wait_with_output()
        .context("waiting for Tailscale API response")?;
    let response_text =
        String::from_utf8(output.stdout).context("Tailscale API response was not UTF-8")?;
    let (payload, status) = response_text
        .rsplit_once('\n')
        .ok_or_else(|| anyhow!("Tailscale API response omitted HTTP status"))?;
    let status: u16 = status
        .parse()
        .context("Tailscale API returned an invalid HTTP status")?;
    if !output.status.success() || !(200..300).contains(&status) {
        let message = serde_json::from_str::<Value>(payload)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        return Err(match message {
            Some(message) => anyhow!("Tailscale API returned HTTP {status}: {message}"),
            None => anyhow!("Tailscale API request failed with HTTP {status}"),
        });
    }
    if payload.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(payload).context("decoding Tailscale API response")
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

fn curl_quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn path_segment(value: &str) -> Result<String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(anyhow!("invalid Tailscale device ID"));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
#[path = "tailscale_tests.rs"]
mod tests;
