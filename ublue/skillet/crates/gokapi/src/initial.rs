//! Native setup/API bootstrap, isolated from all networks; never edits SQLite.
use crate::{Config, Error, Settings, IMAGE};
use serde_json::{json, Value};
use std::{
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{chown, OpenOptionsExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub trait Api {
    fn request(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, Value), Error>;
}

/// Existing users are left untouched; native creation gives new users the User
/// rank. The separately selected initial admin is created by Gokapi's setup.
pub fn accounts(api: &impl Api, settings: &Settings) -> Result<(), Error> {
    let (status, response) = api.request(
        "/auth/token",
        &[
            ("X-Skillet-User", &settings.admin),
            ("permission", "PERM_MANAGE_USERS"),
        ],
        None,
    )?;
    let key = Zeroizing::new(
        response["key"]
            .as_str()
            .filter(|key| !key.is_empty())
            .ok_or(Error::Consumer)?
            .to_string(),
    );
    if status != 200 {
        return Err(Error::Consumer);
    }
    for user in settings
        .users
        .iter()
        .filter(|user| **user != settings.admin)
    {
        let (status, _) = api.request(
            "/api/user/create",
            &[("apikey", &key), ("username", user)],
            Some(""),
        )?;
        if !matches!(status, 200 | 409) {
            return Err(Error::Consumer);
        }
    }
    Ok(())
}

pub fn validate_native(bytes: &[u8], settings: &Settings) -> Result<(), Error> {
    let value: Value = serde_json::from_slice(bytes)?;
    if value["Authentication"]["Method"] != 2
        || value["Authentication"]["HeaderKey"] != "X-Skillet-User"
        || value["Authentication"]["OnlyRegisteredUsers"] != true
        || value["Authentication"]["Username"] != settings.admin
        || value["ServerUrl"] != settings.public_url
        || value["DatabaseUrl"] != "sqlite:///app/config/gokapi.sqlite"
        || value["DataDir"] != "/app/data"
        || value["UseSsl"] != false
    {
        return Err(Error::Binding);
    }
    Ok(())
}

pub fn setup_body(settings: &Settings) -> Result<Zeroizing<String>, Error> {
    settings.validate()?;
    let mut entropy = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    let mut password = Zeroizing::new(String::with_capacity(64));
    for byte in entropy {
        let _ = write!(password, "{byte:02x}");
    }
    let fields = [
        ("localhost_sel", "false"),
        ("ssl_sel", "false"),
        ("logip_sel", "false"),
        ("port", "53842"),
        ("public_name", "Send files"),
        ("url", settings.public_url.as_str()),
        ("url_redirection", "/error"),
        ("showfilename_sel", "false"),
        ("authentication_sel", "2"),
        ("auth_username", "unused"),
        ("auth_pw", password.as_str()),
        ("oauth_provider", ""),
        ("oauth_id", ""),
        ("oauth_secret", ""),
        ("oauth_recheck_interval", "12"),
        ("oauth_admin_user", ""),
        ("oauth_only_registered_users", "true"),
        ("oauth_restrict_groups", "false"),
        ("oauth_allowed_groups", ""),
        ("oauth_scope_groups", ""),
        ("auth_headerkey", "X-Skillet-User"),
        ("auth_header_admin", settings.admin.as_str()),
        ("auth_header_only_registered_users", "true"),
        ("storage_sel", "local"),
        ("storage_sel_image", "local"),
        ("storage_sel_proxy", "false"),
        ("encrypt_sel", "0"),
        ("enc_pw", ""),
        ("dbtype_sel", "0"),
        ("sqlite_location", "/app/config/gokapi.sqlite"),
        ("redis_location", ""),
        ("redis_prefix", ""),
        ("redis_user", ""),
        ("redis_password", ""),
        ("redis_ssl_sel", "false"),
    ];
    Ok(Zeroizing::new(serde_json::to_string(
        &fields
            .iter()
            .map(|(name, value)| json!({"name":name,"value":value}))
            .collect::<Vec<_>>(),
    )?))
}

pub fn initialize(config: Config, settings: &Settings, uid: u32, gid: u32) -> Result<(), Error> {
    config.validate()?;
    settings.validate()?;
    let _lock = lock()?;
    let root = Path::new(config.state_path);
    let native = root.join("config/config.json");
    let marker = root.join(".skillet-initial.json");
    let pending = root.join(".skillet-bootstrap-binding.json");
    if !native.exists() && (marker.exists() || root.join("config/gokapi.sqlite").exists()) {
        return Err(Error::Binding);
    }
    if native.exists() {
        validate_native(&fs::read(&native)?, settings)?;
    }
    if marker.exists() {
        if !root.join("config/gokapi.sqlite").is_file() {
            return Err(Error::Binding);
        }
        if serde_json::from_slice::<Settings>(&fs::read(&marker)?)? == *settings {
            if pending.exists() {
                if serde_json::from_slice::<Settings>(&fs::read(&pending)?)? != *settings {
                    return Err(Error::Binding);
                }
                publish_initial(root, settings)?;
            }
            return Ok(());
        }
    }
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "gokapi.service"])
        .status()?
        .success()
    {
        return Err(Error::Binding);
    }
    if !native.exists() {
        bootstrap_new(root, settings, uid, gid)?;
        return Ok(());
    }
    if pending.exists() {
        let bound: Settings = serde_json::from_slice(&fs::read(&pending)?)?;
        if bound != *settings {
            return Err(Error::Binding);
        }
        publish_initial(root, settings)?;
        return Ok(());
    }
    let bootstrap = Container::start(root, uid, gid)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match accounts(&bootstrap, settings) {
            Ok(()) => break,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(500)),
            Err(error) => return Err(error),
        }
    }
    // Stop the isolated writer before publishing completion or starting the app.
    bootstrap.stop()?;
    publish_marker(root, settings)?;
    Ok(())
}

pub(super) fn lock() -> Result<File, Error> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open("/run/skillet-gokapi.lock")?;
    file.lock()?;
    Ok(file)
}

struct Container {
    name: String,
}
impl Container {
    fn start(state_path: &Path, uid: u32, gid: u32) -> Result<Self, Error> {
        let name = remove_owned(state_path)?;
        let container = Self { name };
        let mut arguments = vec![
            "run",
            "--detach",
            "--name",
            &container.name,
            "--label",
            &format!("io.skillet.gokapi-bootstrap={}", state_path.display()),
            "--network",
            "none",
            "--user",
            &format!("{uid}:{gid}"),
            "--env",
            "GOKAPI_GUEST_UPLOAD_BY_DEFAULT=true",
            "--env",
            "GOKAPI_DATA_DIR=/app/data",
            "--env",
            "GOKAPI_CONFIG_DIR=/app/config",
            "--volume",
            &format!("{}/config:/app/config:Z", state_path.display()),
            "--volume",
            &format!("{}/data:/app/data:Z", state_path.display()),
            IMAGE,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let info = run(&["info", "--format", "{{.Host.Security.Rootless}}"], None)?;
        if !info.status.success() {
            return Err(Error::Consumer);
        }
        match String::from_utf8_lossy(&info.stdout).trim() {
            "true" => {
                arguments.pop();
                arguments.extend(["--userns".into(), "keep-id".into(), IMAGE.into()]);
            }
            "false" => {}
            _ => return Err(Error::Consumer),
        }
        checked(&arguments.iter().map(String::as_str).collect::<Vec<_>>())?;
        Ok(container)
    }
    fn stop(&self) -> Result<(), Error> {
        checked(&["stop", "--time", "10", &self.name])?;
        checked(&["rm", &self.name])
    }
}
impl Drop for Container {
    fn drop(&mut self) {
        let _ = run(&["rm", "--force", &self.name], None);
    }
}
impl Api for Container {
    fn request(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, Value), Error> {
        let mut input = Zeroizing::new(format!(
            "url = {}\n",
            serde_json::to_string(&format!("http://127.0.0.1:53842{path}"))?
        ));
        for (name, value) in headers {
            if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
                return Err(Error::Invalid);
            }
            let _ = writeln!(
                input,
                "header = {}",
                serde_json::to_string(&format!("{name}: {value}"))?
            );
        }
        if let Some(body) = body {
            let _ = writeln!(input, "data-binary = {}", serde_json::to_string(body)?);
        }
        let output = run(
            &[
                "exec",
                "--interactive",
                &self.name,
                "curl",
                "--silent",
                "--show-error",
                "--max-time",
                "10",
                "--max-filesize",
                "1048576",
                "--config",
                "-",
                "--write-out",
                "\n%{http_code}",
            ],
            Some(input.as_bytes()),
        )?;
        if !output.status.success() {
            return Err(Error::Consumer);
        }
        let text = Zeroizing::new(String::from_utf8(output.stdout).map_err(|_| Error::Consumer)?);
        let (body, status) = text.rsplit_once('\n').ok_or(Error::Consumer)?;
        let value = serde_json::from_str(body).unwrap_or(Value::Null);
        Ok((status.parse().map_err(|_| Error::Consumer)?, value))
    }
}
fn checked(arguments: &[&str]) -> Result<(), Error> {
    if run(arguments, None)?.status.success() {
        Ok(())
    } else {
        Err(Error::Consumer)
    }
}
fn run(arguments: &[&str], input: Option<&[u8]>) -> Result<std::process::Output, Error> {
    let mut child = Command::new("timeout")
        .args(["120", "podman"])
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut pipe) = child.stdin.take() {
        if let Some(input) = input {
            pipe.write_all(input)?;
        }
    }
    Ok(child.wait_with_output()?)
}
#[cfg(test)]
mod tests;

pub(super) fn no_bootstrap(config: Config) -> Result<(), Error> {
    for path in [
        Path::new(config.state_path).to_path_buf(),
        Path::new(config.state_path).join(".skillet-bootstrap-state"),
    ] {
        let output = run(
            &[
                "ps",
                "--filter",
                &format!("label=io.skillet.gokapi-bootstrap={}", path.display()),
                "--format",
                "{{.ID}}",
            ],
            None,
        )?;
        if !output.status.success() || !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
            return Err(Error::Consumer);
        }
    }
    Ok(())
}

fn remove_owned(state_path: &Path) -> Result<String, Error> {
    use sha2::{Digest, Sha256};
    let name = format!(
        "skillet-gokapi-bootstrap-{:x}",
        Sha256::digest(state_path.as_os_str().as_encoded_bytes())
    );
    let inspect = run(&["inspect", &name], None)?;
    if inspect.status.success() {
        let inspected: Value = serde_json::from_slice(&inspect.stdout)?;
        let inspected = &inspected[0];
        let mounts = inspected["Mounts"].as_array().ok_or(Error::Binding)?;
        if inspected["Config"]["Labels"]["io.skillet.gokapi-bootstrap"]
            != state_path.to_string_lossy().as_ref()
            || mounts.len() != 2
            || ["config", "data"].iter().any(|leaf| {
                !mounts.iter().any(|mount| {
                    mount["Destination"] == format!("/app/{leaf}")
                        && mount["Source"]
                            == format!("{}/{leaf}", state_path.to_string_lossy().as_ref())
                })
            })
        {
            return Err(Error::Binding);
        }
        checked(&["rm", "--force", &name])?;
    } else {
        let exists = run(&["container", "exists", &name], None)?;
        if exists.status.code() != Some(1) {
            return Err(Error::Consumer);
        }
    }
    Ok(name)
}

fn bootstrap_new(root: &Path, settings: &Settings, uid: u32, gid: u32) -> Result<(), Error> {
    let pending = root.join(".skillet-bootstrap-binding.json");
    if pending.exists() {
        if serde_json::from_slice::<Settings>(&fs::read(&pending)?)? != *settings {
            return Err(Error::Binding);
        }
    } else {
        // Refuse to adopt pre-existing uninitialized application files.
        for leaf in ["config", "data"] {
            if fs::read_dir(root.join(leaf))?.next().is_some() {
                return Err(Error::Binding);
            }
        }
        if root.join(".skillet-bootstrap-state").exists() {
            return Err(Error::Binding);
        }
        let temporary = root.join(".skillet-bootstrap-binding.pending");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(settings)?)?;
        file.sync_all()?;
        fs::hard_link(&temporary, &pending)?;
        fs::remove_file(temporary)?;
        File::open(root)?.sync_all()?;
    }
    let staged = root.join(".skillet-bootstrap-state");
    remove_owned(&staged)?;
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::create_dir(&staged)?;
    for path in [&staged, &staged.join("config"), &staged.join("data")] {
        if path != &staged {
            fs::create_dir(path)?;
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        chown(path, Some(uid), Some(gid))?;
    }
    let bootstrap = Container::start(&staged, uid, gid)?;
    let deadline = Instant::now() + Duration::from_secs(90);
    while bootstrap.request("/", &[], None).is_err() {
        if Instant::now() >= deadline {
            return Err(Error::Consumer);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let body = setup_body(settings)?;
    let (status, result) = bootstrap.request(
        "/setup/setupResult",
        &[("Content-Type", "application/json")],
        Some(&body),
    )?;
    if status != 200 || result["result"] != "OK" {
        return Err(Error::Consumer);
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match accounts(&bootstrap, settings) {
            Ok(()) => break,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(500)),
            Err(error) => return Err(error),
        }
    }
    bootstrap.stop()?;
    validate_native(&fs::read(staged.join("config/config.json"))?, settings)?;
    publish_initial(root, settings)
}

fn publish_initial(root: &Path, settings: &Settings) -> Result<(), Error> {
    let staged = root.join(".skillet-bootstrap-state");
    for leaf in ["config", "data"] {
        let from = staged.join(leaf);
        let to = root.join(leaf);
        if from.exists() {
            if to.exists() {
                if fs::read_dir(&to)?.next().is_some() {
                    return Err(Error::Binding);
                }
                fs::remove_dir(&to)?;
            }
            fs::rename(from, to)?;
            File::open(root)?.sync_all()?;
        } else if !to.is_dir() {
            return Err(Error::Binding);
        }
    }
    validate_native(&fs::read(root.join("config/config.json"))?, settings)?;
    publish_marker(root, settings)?;
    fs::remove_file(root.join(".skillet-bootstrap-binding.json"))?;
    if staged.exists() {
        fs::remove_dir(staged)?;
    }
    File::open(root)?.sync_all()?;
    Ok(())
}
fn publish_marker(root: &Path, settings: &Settings) -> Result<(), Error> {
    let temporary = root.join(".skillet-initial.pending");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(settings)?)?;
    file.sync_all()?;
    fs::rename(temporary, root.join(".skillet-initial.json"))?;
    File::open(root)?.sync_all()?;
    Ok(())
}

/// Resolve the actual dedicated network, rather than trusting auto-assigned
/// bridge ranges or retaining addresses from another boot/VM.
pub fn prepare_proxies(network: &str) -> Result<(), Error> {
    if network.is_empty()
        || !network
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::Invalid);
    }
    let output = run(&["network", "inspect", network], None)?;
    if !output.status.success() {
        return Err(Error::Consumer);
    }
    let proxies = proxy_subnets(network, &output.stdout)?;
    let root = Path::new("/run/skillet/gokapi");
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    let temporary = root.join("proxy.pending");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    writeln!(file, "GOKAPI_TRUSTED_PROXIES={}", proxies.join(","))?;
    file.sync_all()?;
    fs::rename(temporary, root.join("proxy.env"))?;
    Ok(())
}
fn proxy_subnets(network: &str, bytes: &[u8]) -> Result<Vec<String>, Error> {
    let values: Vec<Value> = serde_json::from_slice(bytes)?;
    if values.len() != 1 {
        return Err(Error::Binding);
    }
    let value = &values[0];
    if value["name"] != network
        || value["driver"] != "bridge"
        || value["internal"] != false
        || value["options"]["isolate"] != "strict"
    {
        return Err(Error::Binding);
    }
    let proxies = value["subnets"]
        .as_array()
        .ok_or(Error::Binding)?
        .iter()
        .map(|subnet| {
            subnet["subnet"]
                .as_str()
                .filter(|value| crate::valid_proxy(value))
                .map(str::to_owned)
                .ok_or(Error::Binding)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if proxies.is_empty() {
        return Err(Error::Binding);
    }
    Ok(proxies)
}
