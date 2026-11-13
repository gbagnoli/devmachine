//! Bounded, credential-free readiness for the local `UniFi` controller API.
use reqwest::{blocking::Client, header::CONTENT_TYPE, StatusCode, Url};
use std::{
    io::Read as _,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("readiness URL must be a credential-free loopback HTTP(S) /status endpoint")]
    Endpoint,
    #[error("readiness timeout must be between 1 and 180 seconds")]
    Timeout,
    #[error("could not create local readiness client")]
    Client,
    #[error("UniFi API did not become ready before the deadline")]
    NotReady,
}

fn validate_endpoint(endpoint: &str) -> Result<Url, Error> {
    let url = Url::parse(endpoint).map_err(|_| Error::Endpoint)?;
    let loopback = url
        .host_str()
        .and_then(|host| {
            host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .is_some_and(|ip| ip.is_loopback());
    if !loopback
        || !matches!(url.scheme(), "http" | "https")
        || url.path() != "/status"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Endpoint);
    }
    Ok(url)
}

fn ready(status: StatusCode, content_type: &str, body: &[u8]) -> bool {
    if status != StatusCode::OK
        || !content_type
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
        || body.len() > 16384
    {
        return false;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    value["meta"]["rc"] == "ok" && value["meta"]["up"] == true
}

/// Only a loopback endpoint may use the controller's self-signed certificate.
/// Redirects/proxies are disabled and neither credentials nor response bodies
/// are printed. Each attempt shares the monotonic overall deadline.
pub fn wait(endpoint: &str, timeout: Duration) -> Result<(), Error> {
    let url = validate_endpoint(endpoint)?;
    if timeout < Duration::from_secs(1) || timeout > Duration::from_secs(180) {
        return Err(Error::Timeout);
    }
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|_| Error::Client)?;
    let deadline = Instant::now() + timeout;
    wait_with(timeout, Duration::from_secs(2), |remaining| {
        let Ok(response) = client
            .get(url.clone())
            .timeout(remaining.min(Duration::from_secs(5)))
            .send()
        else {
            return false;
        };
        let status = response.status();
        let kind = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let mut body = Vec::new();
        if response.take(16385).read_to_end(&mut body).is_err() {
            return false;
        }
        Instant::now() < deadline && ready(status, &kind, &body)
    })
}

fn wait_with(
    timeout: Duration,
    interval: Duration,
    mut probe: impl FnMut(Duration) -> bool,
) -> Result<(), Error> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::NotReady);
        }
        if probe(remaining) && Instant::now() < deadline {
            return Ok(());
        }
        std::thread::sleep(interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
