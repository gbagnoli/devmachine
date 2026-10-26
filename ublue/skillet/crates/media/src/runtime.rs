//! Native guest adapter: inspect only network metadata and use bounded HTTP.
use crate::{
    reconcile::{Api, Method},
    App, Config, Credentials, Error,
};
use reqwest::blocking::Client;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read as _,
    net::IpAddr,
    process::Command,
    time::{Duration, Instant},
};

pub struct HttpApi<'a> {
    client: Client,
    endpoints: BTreeMap<App, String>,
    credentials: &'a Credentials,
}

impl<'a> HttpApi<'a> {
    pub fn new(
        endpoints: BTreeMap<App, String>,
        credentials: &'a Credentials,
    ) -> Result<Self, Error> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| Error::Invalid("HTTP client"))?;
        Ok(Self {
            client,
            endpoints,
            credentials,
        })
    }
}

impl Api for HttpApi<'_> {
    fn request(
        &self,
        app: App,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, Error> {
        let endpoint = self
            .endpoints
            .get(&app)
            .ok_or(Error::Invalid("application endpoint"))?;
        let url = format!("{endpoint}/{}/{path}", app.api());
        let operation = match method {
            Method::Get => "read",
            Method::Post => "create/test",
            Method::Put => "update",
        };
        let failure = || Error::Transport {
            app: app.name(),
            operation,
        };
        let mut request = self
            .client
            .request(
                match method {
                    Method::Get => reqwest::Method::GET,
                    Method::Post => reqwest::Method::POST,
                    Method::Put => reqwest::Method::PUT,
                },
                url,
            )
            .header("X-Api-Key", self.credentials.key(app)?);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().map_err(|_| failure())?;
        if !response.status().is_success() {
            return Err(Error::Api {
                app: app.name(),
                operation,
                status: response.status().as_u16(),
            });
        }
        let mut bytes = Vec::new();
        response
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(failure());
        }
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| failure())
    }
}

pub fn run(config: Config, credentials: &Credentials) -> Result<(), Error> {
    config.validate()?;
    credentials.validate()?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let api = loop {
        match endpoints(config).and_then(|endpoints| HttpApi::new(endpoints, credentials)) {
            Ok(api)
                if App::ARR.iter().all(|app| {
                    api.request(*app, Method::Get, "system/status", None)
                        .is_ok()
                }) =>
            {
                break api
            }
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_secs(2)),
            _ => {
                return Err(Error::Invalid(
                    "applications did not become ready within 180 seconds",
                ))
            }
        }
    };
    crate::reconcile::reconcile(&api, credentials)
}

fn endpoints(config: Config) -> Result<BTreeMap<App, String>, Error> {
    App::ARR
        .into_iter()
        .map(|app| {
            let output = Command::new("podman")
                .args([
                    "inspect",
                    "--format",
                    "{{json .NetworkSettings.Networks}}",
                    app.name(),
                ])
                .output()?;
            if !output.status.success() {
                return Err(Error::Invalid("application container network"));
            }
            let networks: Value = serde_json::from_slice(&output.stdout)
                .map_err(|_| Error::Invalid("container network metadata"))?;
            let address = networks[config.network]["IPAddress"]
                .as_str()
                .and_then(|address| address.parse::<IpAddr>().ok())
                .ok_or(Error::Invalid("application bridge address"))?;
            let host = match address {
                IpAddr::V4(address) => address.to_string(),
                IpAddr::V6(address) => format!("[{address}]"),
            };
            Ok((app, format!("http://{host}:{}", app.port())))
        })
        .collect()
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
