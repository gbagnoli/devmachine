//! Real consumer contracts, run only through the owned disposable VM adapter.
use serde_json::Value;
use skillet_core::files::{FileMutationResource, LocalFileResource};
use skillet_media::{
    reconcile::{self, Api, Method},
    runtime::HttpApi,
    App, Config, Credentials, Error,
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    io,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Sandbox {
    root: Option<tempfile::TempDir>,
    network: String,
    containers: Vec<String>,
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let mut clean = true;
        for name in self.containers.iter().rev() {
            clean &= podman(&["rm", "-f", name]).is_ok();
        }
        clean &= podman(&["network", "rm", &self.network]).is_ok();
        if !clean {
            if let Some(root) = self.root.take() {
                eprintln!(
                    "Fixture cleanup incomplete; retained {}",
                    root.keep().display()
                );
            }
        }
    }
}

fn podman(args: &[&str]) -> Result<String> {
    let output = Command::new("timeout")
        .arg("600")
        .arg("podman")
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "fixture Podman operation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

struct Counted<'a> {
    api: HttpApi<'a>,
    endpoints: BTreeMap<App, String>,
    credentials: &'a Credentials,
    writes: Cell<usize>,
}
impl Api for Counted<'_> {
    fn request(
        &self,
        app: App,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> std::result::Result<Value, Error> {
        if matches!(method, Method::Put)
            || (matches!(method, Method::Post) && !path.ends_with("/test"))
        {
            self.writes.set(self.writes.get() + 1);
        }
        if matches!(method, Method::Put) {
            let collection = path.split('/').next().unwrap_or_default();
            if let (Ok(current), Some(desired)) =
                (self.api.request(app, Method::Get, collection, None), body)
            {
                if let Some(existing) = current
                    .as_array()
                    .and_then(|items| items.iter().find(|value| value["id"] == desired["id"]))
                {
                    for property in ["syncLevel", "enable", "priority"] {
                        if existing[property] != desired[property] {
                            eprintln!("Fixture update {app:?}: changed property {property}");
                        }
                    }
                    if let Some(fields) = desired["fields"].as_array() {
                        for field in fields {
                            if let Some(previous) =
                                existing["fields"].as_array().and_then(|values| {
                                    values.iter().find(|value| value["name"] == field["name"])
                                })
                            {
                                if previous["value"] != field["value"] {
                                    let masked = previous["value"].as_str().is_some_and(|v| {
                                        !v.is_empty() && v.chars().all(|c| c == '*')
                                    });
                                    eprintln!("Fixture update {app:?}: changed field {} (masked={masked})",field["name"].as_str().unwrap_or("unknown"));
                                }
                            }
                        }
                    }
                }
            }
        }
        let result = self.api.request(app, method, path, body);
        // This adapter is only used with synthetic keys and empty fixture apps.
        // Repeat only nonmutating connection tests to collect consumer validation
        // diagnostics; production HttpApi never returns or logs response bodies.
        if result.is_err() && path.ends_with("/test") {
            if let Ok(response) = reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(20))
                .build()
                .and_then(|client| {
                    client
                        .post(format!("{}/{}/{path}", self.endpoints[&app], app.api()))
                        .header("X-Api-Key", self.credentials.key(app).unwrap_or_default())
                        .json(&body)
                        .send()
                })
            {
                eprintln!(
                    "Synthetic fixture validation for {app:?} {path}: {}",
                    response.text().unwrap_or_default()
                );
            }
        }
        result
    }
}

fn config(root: &Path, network: String, owner: String, group: String) -> Config {
    let permanent = |path: std::path::PathBuf| -> &'static str {
        Box::leak(path.to_string_lossy().into_owned().into_boxed_str())
    };
    Config {
        config_root: permanent(root.join("config")),
        files_path: permanent(root.join("files")),
        music_path: permanent(root.join("music")),
        network: Box::leak(network.into_boxed_str()),
        data_owner: Box::leak(owner.into_boxed_str()),
        data_group: Box::leak(group.into_boxed_str()),
    }
}

fn start(
    sandbox: &mut Sandbox,
    config: Config,
    credentials: &Credentials,
) -> Result<BTreeMap<App, String>> {
    let files = LocalFileResource::new();
    let (uid, gid) = config.identity(&skillet_core::system::LinuxSystemResource::new())?;
    for path in [
        config.files_path.to_string(),
        format!("{}/library/movies", config.files_path),
        format!("{}/library/tv", config.files_path),
        config.music_path.into(),
    ] {
        files.ensure_directory(
            Path::new(&path),
            Some(0o2775),
            &skillet_core::files::Ownership::named(
                Some(config.data_owner),
                Some(config.data_group),
            ),
        )?;
    }
    let mut endpoints = BTreeMap::new();
    for app in App::ARR {
        skillet_media::bootstrap::ensure(&files, config, app, credentials.key(app)?)?;
        podman(&["pull", app.image()])?;
        let name = format!("{}-{}", sandbox.network, app.name());
        podman(&[
            "run",
            "-d",
            "--name",
            &name,
            "--network",
            &sandbox.network,
            "--network-alias",
            app.name(),
            "-e",
            &format!("PUID={uid}"),
            "-e",
            &format!("PGID={gid}"),
            "-v",
            &format!("{}:/config:z", config.config_path(app).display()),
            "-v",
            &format!("{}:/data:z", config.files_path),
            "-v",
            &format!("{}:/music:z", config.music_path),
            app.image(),
        ])?;
        sandbox.containers.push(name.clone());
        let data: Value = serde_json::from_str(&podman(&[
            "inspect",
            "--format",
            "{{json .NetworkSettings.Networks}}",
            &name,
        ])?)?;
        let address = data[&sandbox.network]["IPAddress"]
            .as_str()
            .ok_or("fixture bridge address absent")?;
        endpoints.insert(app, format!("http://{address}:{}", app.port()));
        println!("Started {} from {}", app.name(), app.image());
        let bindings: Value = serde_json::from_str(&podman(&[
            "inspect",
            "--format",
            "{{json .HostConfig.PortBindings}}",
            &name,
        ])?)?;
        assert!(bindings.is_null() || bindings.as_object().is_some_and(serde_json::Map::is_empty));
    }
    Ok(endpoints)
}

fn wait(api: &impl Api) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(240);
    loop {
        if App::ARR.iter().all(|app| {
            api.request(*app, Method::Get, "system/status", None)
                .is_ok()
        }) {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("Arr consumers did not become ready within 240 seconds".into());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn bridge_endpoints(sandbox: &Sandbox) -> Result<BTreeMap<App, String>> {
    App::ARR
        .into_iter()
        .map(|app| {
            let name = format!("{}-{}", sandbox.network, app.name());
            let data: Value = serde_json::from_str(&podman(&[
                "inspect",
                "--format",
                "{{json .NetworkSettings.Networks}}",
                &name,
            ])?)?;
            let address = data[&sandbox.network]["IPAddress"]
                .as_str()
                .ok_or("fixture bridge address absent")?;
            Ok((app, format!("http://{address}:{}", app.port())))
        })
        .collect()
}

#[test]
#[ignore = "requires explicit owned disposable VM; use the media_consumer workstation example"]
fn arr_consumer_contract() -> Result<()> {
    let expected = std::env::var("SKILLET_DISPOSABLE_HOSTNAME")?;
    let hostname = std::fs::read_to_string("/etc/hostname")?;
    if hostname.trim() != expected || !expected.contains("-test-") {
        return Err("requires the manifest-bound disposable hostname".into());
    }
    let owner = std::env::var("SKILLET_DISPOSABLE_DATA_OWNER")?;
    let group = std::env::var("SKILLET_DISPOSABLE_DATA_GROUP")?;
    let root = tempfile::Builder::new()
        .prefix("skillet-media-contract-")
        .tempdir_in("/var/lib/data")?;
    let network = format!("skillet-media-contract-{}", std::process::id());
    podman(&[
        "network",
        "create",
        "--ipv6",
        "--opt",
        "isolate=strict",
        &network,
    ])?;
    let config = config(root.path(), network.clone(), owner, group);
    let mut sandbox = Sandbox {
        root: Some(root),
        network,
        containers: Vec::new(),
    };
    let credentials = Credentials {
        api_keys: App::ARR
            .into_iter()
            .enumerate()
            .map(|(n, app)| (app, format!("{:032x}", n + 1)))
            .collect(),
        putio_password: "FixturePassword".repeat(4),
        putio_token: "unused-no-provider-requests".into(),
    };
    let endpoints = start(&mut sandbox, config, &credentials)?;
    let api = Counted {
        api: HttpApi::new(endpoints.clone(), &credentials)?,
        endpoints: endpoints.clone(),
        credentials: &credentials,
        writes: Cell::new(0),
    };
    wait(&api)?;
    authentication(&api, &endpoints, &credentials)?;
    println!("Checking fresh Prowlarr links and library roots");
    reconcile::reconcile_applications(&api, &credentials)?;
    let first = api.writes.get();
    println!("Checking repeat convergence");
    reconcile::reconcile_applications(&api, &credentials)?;
    assert_eq!(
        api.writes.get(),
        first,
        "unchanged consumer state caused new API writes"
    );
    let applications = api.request(App::Prowlarr, Method::Get, "applications", None)?;
    assert_eq!(
        applications.as_array().ok_or("application schema")?.len(),
        3
    );
    println!("Checking consumer restart persistence");
    for name in &sandbox.containers {
        podman(&["restart", name])?;
    }
    let endpoints = bridge_endpoints(&sandbox)?;
    let after = Counted {
        api: HttpApi::new(endpoints.clone(), &credentials)?,
        endpoints,
        credentials: &credentials,
        writes: Cell::new(first),
    };
    wait(&after)?;
    reconcile::reconcile_applications(&after, &credentials)?;
    assert_eq!(
        after.writes.get(),
        first,
        "restart lost linked consumer state"
    );
    println!("PASS: four real Arr consumers, API-key enforcement, External browser auth, three Prowlarr connection tests, library roots, repeat convergence and container restart persistence");
    Ok(())
}

fn authentication(
    api: &impl Api,
    endpoints: &BTreeMap<App, String>,
    credentials: &Credentials,
) -> Result<()> {
    for app in App::ARR {
        let status = api.request(app, Method::Get, "system/status", None)?;
        println!(
            "Ready {} {}",
            app.name(),
            status["version"].as_str().unwrap_or("unknown")
        );
        let mut wrong = Credentials {
            api_keys: credentials.api_keys.clone(),
            putio_password: credentials.putio_password.clone(),
            putio_token: credentials.putio_token.clone(),
        };
        wrong.api_keys.insert(app, "f".repeat(32));
        assert!(
            matches!(
                HttpApi::new(endpoints.clone(), &wrong)?.request(
                    app,
                    Method::Get,
                    "system/status",
                    None
                ),
                Err(Error::Api {
                    status: 401 | 403,
                    ..
                })
            ),
            "{app:?} did not explicitly reject the invalid API key"
        );
        let host = api.request(app, Method::Get, "config/host", None)?;
        assert!(
            host["authenticationMethod"]
                .as_str()
                .is_some_and(|method| method.eq_ignore_ascii_case("external")),
            "{app:?} did not retain External authentication"
        );
        // A browser request has no API key and must reach HTML through External auth.
        let browser = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?
            .get(&endpoints[&app])
            .send()?;
        assert!(
            browser.status().is_success(),
            "{app:?} still requires a native browser login"
        );
    }
    Ok(())
}
