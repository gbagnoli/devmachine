//! Guest-only synthetic acceptance. Run through the locked `guest_command` VM adapter.
use anyhow::{bail, ensure, Context, Result};
use reqwest::blocking::Client;
use serde_json::json;
use skillet_core::{
    files::{FileMutationResource, LocalFileResource, Ownership, StorageResource},
    system::{LinuxSystemResource, ServiceResource},
};
use skillet_homeassistant::backup::{self, Api, Settings};
use skillet_podman::{
    MountDependency, PodmanConfig, PortProtocol, PortPublication, ProcessIdentity, Volume,
};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    net::Ipv4Addr,
    path::Path,
    time::{Duration, Instant},
};
#[derive(clap::Parser)]
struct Args {
    #[arg(long)]
    verify: bool,
}
fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = <Args as clap::Parser>::parse();
    let host = fs::read_to_string("/etc/skillet/host")?;
    let runtime = fs::read_to_string("/etc/hostname")?;
    ensure!(
        runtime
            .trim()
            .starts_with(&format!("{}-test-", host.trim())),
        "requires disposable runtime identity"
    );
    ensure!(
        std::process::Command::new("systemd-detect-virt")
            .arg("--vm")
            .status()?
            .success(),
        "requires VM"
    );
    let profile = skillet_hosts::profile_for_name(host.trim()).context("unknown profile")?;
    let config = profile
        .services
        .iter()
        .find_map(|s| match s.config {
            skillet_hosts::ServiceConfig::HomeAssistant(c) => Some(c),
            _ => None,
        })
        .context("Home Assistant not declared")?;
    let files = LocalFileResource;
    let system = LinuxSystemResource::new();
    if !args.verify {
        files.ensure_btrfs_subvolume(Path::new(config.backup_subvolume))?;
        skillet_homeassistant::apply(&system, &files, config)?;
    }
    wait_http()?;
    check_listener()?;
    if !args.verify {
        prepare_proxy(&system, &files, &profile)?;
    }
    verify_https()?;
    let path = Path::new("/var/lib/skillet/homeassistant/acceptance.json");
    let input = if path.exists() {
        Settings::parse(&fs::read_to_string(path)?)?
    } else {
        ensure!(!args.verify, "missing synthetic account");
        let token = onboard()?;
        let input = Settings {
            version: 1,
            host: host.trim().into(),
            runtime_hostname: runtime.trim().into(),
            api_token: token,
            backup_password: random()?,
        };
        files.ensure_file(
            path,
            &serde_json::to_vec(&input)?,
            Some(0o600),
            &Ownership::named(Some("root"), Some("root")),
        )?;
        input
    };
    let marker = Path::new(skillet_homeassistant::DATA).join("config/skillet-acceptance.txt");
    if !args.verify {
        files.create_file_if_absent(
            &marker,
            b"Home Assistant synthetic persistence marker\n",
            Some(0o600),
            &Ownership::named(Some("root"), Some("root")),
        )?;
    }
    ensure!(
        fs::read(&marker)? == b"Home Assistant synthetic persistence marker\n",
        "state marker changed"
    );
    verify_private_api_https(&input.api_token)?;
    install_credential(host.trim(), &input)?;
    system.service_start(backup::UNIT)?;
    let entries: Vec<_> = fs::read_dir(config.backup_files)?.collect::<std::io::Result<_>>()?;
    ensure!(
        entries
            .iter()
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tar")),
        "encrypted export missing"
    );
    let count = entries.len();
    system.service_start(backup::UNIT)?;
    ensure!(
        fs::read_dir(config.backup_files)?.count() == count,
        "repeated export duplicated archives"
    );
    let mut api = backup::LocalApi::connect(&input.api_token)?;
    let info = api.request(json!({"type":"backup/info"}))?;
    let completed = backup::completed(&info)?;
    ensure!(!completed.is_empty(), "completed backup missing");
    let wrong=api.request(json!({"type":"backup/can_decrypt_on_download","backup_id":completed[0].backup_id,"agent_id":"backup.local","password":"incorrect disposable recovery key"}));
    ensure!(wrong.is_err(), "wrong recovery key accepted");
    if !args.verify {
        skillet_homeassistant::apply(&system, &files, config)?;
    }
    println!("Accepted: loopback listener, confined Caddy HTTPS relay, native account/API authentication, encrypted native backup/export, repeat convergence, wrong-key refusal and state marker{}",if args.verify{" after reboot"}else{""});
    Ok(())
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
fn wait_http() -> Result<()> {
    let client = client()?;
    let deadline = Instant::now() + Duration::from_mins(5);
    while Instant::now() < deadline {
        if client
            .get("http://127.0.0.1:8123/api/onboarding")
            .send()
            .is_ok_and(|r| r.status().is_success())
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    bail!("Home Assistant readiness timeout")
}
fn random() -> Result<String> {
    let mut bytes = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex::encode(bytes))
}
fn onboard() -> Result<String> {
    let client = client()?;
    let password = random()?;
    let base = "http://127.0.0.1:8123/";
    let response:serde_json::Value=client.post(format!("{base}api/onboarding/users")).json(&json!({"client_id":base,"name":"Skillet disposable fixture","username":"skillet-fixture","password":password,"language":"en"})).send()?.error_for_status()?.json()?;
    let code = response["auth_code"]
        .as_str()
        .context("onboarding authorization code absent")?;
    let response: serde_json::Value = client
        .post(format!("{base}auth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", base),
        ])
        .send()?
        .error_for_status()?
        .json()?;
    let token = response["access_token"]
        .as_str()
        .context("onboarding token absent")?;
    let mut api = backup::LocalApi::connect(token)?;
    let result=api.request(json!({"type":"auth/long_lived_access_token","client_name":"Skillet disposable backup acceptance","lifespan":1}))?;
    Ok(result.as_str().context("long-lived token absent")?.into())
}
fn prepare_proxy(
    system: &LinuxSystemResource,
    files: &LocalFileResource,
    profile: &skillet_hosts::HostProfile,
) -> Result<()> {
    let services = profile
        .ui_services()
        .into_iter()
        .filter(|s| s.name == "homeassistant")
        .collect::<Vec<_>>();
    let sites = skillet_caddy::CaddySites::from_host(
        profile.id.as_str(),
        &skillet_caddy::UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )?;
    let mut volumes = vec![];
    skillet_caddy::host_proxy::apply(system, files, &sites, &mut volumes)?;
    let root = Ownership::named(Some("root"), Some("root"));
    let dir = Path::new("/var/lib/data/homeassistant/fixture-proxy");
    files.ensure_directory(dir, Some(0o700), &root)?;
    files.ensure_file(&dir.join("Caddyfile"),b"{\n admin off\n}\nhttps://localhost {\n tls internal\n reverse_proxy unix//run/skillet-ui-host/homeassistant.sock\n}\n",Some(0o644),&root)?;
    volumes.extend([
        Volume {
            host_path: dir.display().to_string(),
            container_path: "/etc/caddy".into(),
            options: Some("ro,Z".into()),
            host_mode: None,
            host_ownership: None,
        },
        Volume {
            host_path: dir.join("data").display().to_string(),
            container_path: "/data".into(),
            options: Some("Z".into()),
            host_mode: Some(0o700),
            host_ownership: None,
        },
    ]);
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "homeassistant-test-proxy".into(),
            image: "ghcr.io/caddybuilds/caddy-cloudflare:2".into(),
            network_attachments: vec![],
            port_publications: vec![PortPublication {
                host_address: Ipv4Addr::LOCALHOST.into(),
                host_port: 18443,
                container_port: 443,
                protocol: PortProtocol::Tcp,
            }],
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes,
            secrets: vec![],
            config_revisions: vec![],
            extra_config: BTreeMap::from([
                ("Service".into(), vec!["Restart=on-failure".into()]),
                (
                    "Unit".into(),
                    vec![
                        "After=skillet-ui-host-homeassistant.service".into(),
                        "Requires=skillet-ui-host-homeassistant.service".into(),
                    ],
                ),
                ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
            ]),
        },
    )?;
    Ok(())
}
fn verify_private_api_https(token: &str) -> Result<()> {
    let ca = fs::read(
        "/var/lib/data/homeassistant/fixture-proxy/data/caddy/pki/authorities/local/root.crt",
    )?;
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .add_root_certificate(reqwest::Certificate::from_pem(&ca)?)
        .build()?;
    ensure!(
        client
            .get("https://localhost:18443/api/")
            .send()?
            .status()
            .as_u16()
            == 401,
        "native API accepted an anonymous client"
    );
    ensure!(
        client
            .get("https://localhost:18443/api/")
            .bearer_auth(token)
            .send()?
            .status()
            .is_success(),
        "authenticated native API failed through HTTPS"
    );
    let response = client
        .get("https://localhost:18443/api/websocket")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Origin", "https://localhost:18443")
        .send()?;
    ensure!(
        response.status().as_u16() == 101,
        "WebSocket upgrade failed through HTTPS"
    );
    Ok(())
}
fn verify_https() -> Result<()> {
    let ca = Path::new(
        "/var/lib/data/homeassistant/fixture-proxy/data/caddy/pki/authorities/local/root.crt",
    );
    let deadline = Instant::now() + Duration::from_mins(2);
    while Instant::now() < deadline {
        if let Ok(pem) = fs::read(ca) {
            let client = Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .add_root_certificate(reqwest::Certificate::from_pem(&pem)?)
                .build()?;
            if client
                .get("https://localhost:18443/")
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    bail!("fixture HTTPS relay failed")
}

fn install_credential(host: &str, input: &Settings) -> Result<()> {
    let payload = serde_json::to_vec(input)?;
    let executable = format!("/var/usrlocal/bin/skillet-{host}");
    let mut process = std::process::Command::new(executable)
        .args([
            "credential",
            "install",
            backup::CREDENTIAL,
            backup::UNIT,
            "--no-start",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    {
        let mut stdin = process.stdin.take().context("credential stdin absent")?;
        std::io::Write::write_all(&mut stdin, &payload)?;
    }
    ensure!(
        process.wait_with_output()?.status.success(),
        "fixture credential installation failed"
    );
    Ok(())
}

fn check_listener() -> Result<()> {
    let http: serde_json::Value = serde_json::from_slice(&fs::read(
        Path::new(skillet_homeassistant::DATA).join("config/.storage/http"),
    )?)?;
    ensure!(
        http["data"]["pending"].is_null()
            && http["data"]["stable"]["server_host"] == json!(["127.0.0.1"])
            && http["data"]["stable"]["use_x_forwarded_for"] == true,
        "HTTP bootstrap is not a confirmed private policy"
    );
    let raw = std::process::Command::new("ss")
        .args(["-H", "-lnt", "sport", "=", "8123"])
        .output()?;
    ensure!(raw.status.success(), "listener inspection failed");
    let listeners = String::from_utf8(raw.stdout)?;
    ensure!(
        listeners.contains("127.0.0.1:8123")
            && !listeners.contains("0.0.0.0:8123")
            && !listeners.contains("[::]:8123"),
        "HTTP listener not isolated"
    );
    Ok(())
}
