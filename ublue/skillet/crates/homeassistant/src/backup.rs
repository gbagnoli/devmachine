//! Native backup scheduling and completed encrypted archive publication.
use crate::{Config, Error};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skillet_core::{
    files::{FileMutationResource, Ownership},
    system::ServiceResource,
};
use std::os::fd::AsRawFd;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const CREDENTIAL: &str = "homeassistant_backup";
pub const UNIT: &str = "skillet-homeassistant-backup.service";
pub const TIMER: &str = "skillet-homeassistant-backup.timer";

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u8,
    pub host: String,
    pub runtime_hostname: String,
    pub api_token: String,
    pub backup_password: String,
}
pub fn valid_token(value: &str) -> bool {
    value.len() >= 32 && value.len() <= 16384 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub fn valid_password(value: &str) -> bool {
    (28..=256).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_graphic())
}

impl Settings {
    pub fn parse(contents: &str) -> Result<Self, Error> {
        let input: Self = serde_json::from_str(contents).map_err(|_| Error::Invalid)?;
        if input.version != 1
            || input.host.is_empty()
            || input.runtime_hostname.is_empty()
            || !valid_token(&input.api_token)
            || !valid_password(&input.backup_password)
        {
            return Err(Error::Invalid);
        }
        Ok(input)
    }
}

pub fn install<S, F>(system: &S, files: &F, config: Config) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + ?Sized,
{
    config.validate()?;
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
    files.ensure_directory(
        Path::new("/var/lib/skillet/homeassistant"),
        Some(0o700),
        &root,
    )?;
    let owner = Ownership::named(Some(config.owner), Some(config.group));
    let mut directory = PathBuf::from(config.files_root);
    files.ensure_directory(&directory, None, &owner)?;
    let relative = Path::new(config.backup_files)
        .strip_prefix(config.files_root)
        .map_err(|_| Error::Invalid)?;
    for component in relative.components() {
        directory.push(component);
        files.ensure_directory(&directory, Some(0o700), &owner)?;
    }
    let unit = format!("[Unit]\nDescription=Export completed encrypted Home Assistant backups\nAfter=homeassistant.service\nRequiresMountsFor=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/{CREDENTIAL}.cred\n[Service]\nType=oneshot\nLoadCredentialEncrypted={CREDENTIAL}:/etc/credstore.encrypted/skillet/{CREDENTIAL}.cred\nExecStart=/var/usrlocal/bin/skillet homeassistant backup --host-file /etc/skillet/host\nTimeoutStartSec=600\nUMask=0077\nNoNewPrivileges=yes\nProtectHome=yes\nPrivateTmp=yes\n[Install]\nWantedBy=multi-user.target\n");
    let timer = "[Unit]\nDescription=Check Home Assistant backup publication hourly\n[Timer]\nOnBootSec=10min\nOnUnitActiveSec=1h\nRandomizedDelaySec=5min\nPersistent=yes\n[Install]\nWantedBy=timers.target\n";
    let a = files.ensure_file(
        &Path::new("/etc/systemd/system").join(UNIT),
        unit.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let b = files.ensure_file(
        &Path::new("/etc/systemd/system").join(TIMER),
        timer.as_bytes(),
        Some(0o644),
        &root,
    )?;
    if a || b {
        system.daemon_reload()?;
    }
    system.service_enable(TIMER)?;
    system.service_start(TIMER)?;
    Ok(())
}

/// API capability shared by fixtures and the bounded guest WebSocket adapter.
pub trait Api {
    fn request(&mut self, command: Value) -> Result<Value, Error>;
}

pub struct LocalApi {
    socket: tungstenite::WebSocket<TcpStream>,
    next: u64,
}
impl LocalApi {
    pub fn connect(token: &str) -> Result<Self, Error> {
        let tcp = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, crate::PORT)),
            Duration::from_secs(5),
        )?;
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(10)))?;
        let (socket, _) = tungstenite::client::client_with_config(
            "ws://127.0.0.1:8123/api/websocket",
            tcp,
            Some(
                tungstenite::protocol::WebSocketConfig::default()
                    .max_message_size(Some(4 * 1024 * 1024))
                    .max_frame_size(Some(1024 * 1024)),
            ),
        )
        .map_err(|_| Error::Api)?;
        let mut client = Self { socket, next: 0 };
        if client.read()?.get("type").and_then(Value::as_str) != Some("auth_required") {
            return Err(Error::Api);
        }
        client.send(&json!({"type":"auth","access_token":token}))?;
        if client.read()?.get("type").and_then(Value::as_str) != Some("auth_ok") {
            return Err(Error::Api);
        }
        Ok(client)
    }
    fn send(&mut self, value: &Value) -> Result<(), Error> {
        self.socket
            .send(tungstenite::Message::Text(value.to_string().into()))
            .map_err(|_| Error::Api)
    }
    fn read(&mut self) -> Result<Value, Error> {
        for _ in 0..16 {
            match self.socket.read().map_err(|_| Error::Api)? {
                tungstenite::Message::Text(s) if s.len() <= 4 * 1024 * 1024 => {
                    return serde_json::from_str(&s).map_err(|_| Error::Api)
                }
                tungstenite::Message::Ping(_) | tungstenite::Message::Pong(_) => {}
                _ => return Err(Error::Api),
            }
        }
        Err(Error::Api)
    }
}
impl Api for LocalApi {
    fn request(&mut self, mut command: Value) -> Result<Value, Error> {
        self.next += 1;
        command["id"] = json!(self.next);
        self.send(&command)?;
        let response = self.read()?;
        if response.get("id").and_then(Value::as_u64) != Some(self.next)
            || response.get("success") != Some(&Value::Bool(true))
        {
            return Err(Error::Api);
        }
        response.get("result").cloned().ok_or(Error::Api)
    }
}

/// Configure native scheduling only while unconfigured; preserve subsequent application settings.
pub fn configure(api: &mut impl Api, input: &Settings) -> Result<(), Error> {
    let response = api.request(json!({"type":"backup/config/info"}))?;
    let config = &response["config"];
    if config["automatic_backups_configured"] == true {
        if config["create_backup"]["password"].as_str() != Some(input.backup_password.as_str())
            || config["agents"]["backup.local"]["protected"] != true
            || !config["create_backup"]["agent_ids"]
                .as_array()
                .is_some_and(|ids| ids.contains(&json!("backup.local")))
        {
            return Err(Error::Invalid);
        }
        return Ok(());
    }
    api.request(json!({"type":"backup/config/update", "automatic_backups_configured":true,
        "agents":{"backup.local":{"protected":true,"retention":{"copies":7}}},
        "create_backup":{"agent_ids":["backup.local"],"include_database":true,"password":input.backup_password,"name":"Skillet Home Assistant"},
        "retention":{"copies":7},"schedule":{"recurrence":"daily","time":"03:00:00"}}))?;
    Ok(())
}

#[derive(Clone, Deserialize)]
pub struct Completed {
    pub backup_id: String,
    pub size: u64,
}

pub fn completed(response: &Value) -> Result<Vec<Completed>, Error> {
    if response["state"] != "idle"
        || response["agent_errors"]
            .as_object()
            .is_none_or(|errors| !errors.is_empty())
    {
        return Err(Error::Invalid);
    }
    let backups = response["backups"].as_array().ok_or(Error::Invalid)?;
    let mut result = Vec::new();
    for backup in backups {
        let local = &backup["agents"]["backup.local"];
        if local.is_null() {
            continue;
        }
        if local["protected"] != true
            || backup["homeassistant_included"] != true
            || backup["database_included"] != true
            || ["failed_addons", "failed_agent_ids", "failed_folders"]
                .iter()
                .any(|k| backup[*k].as_array().is_none_or(|v| !v.is_empty()))
        {
            return Err(Error::Invalid);
        }
        let id = backup["backup_id"].as_str().ok_or(Error::Invalid)?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::Invalid);
        }
        result.push(Completed {
            backup_id: id.into(),
            size: local["size"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid)?,
        });
    }
    Ok(result)
}

/// Archive storage capability; consumer metadata determines completion, never file age.
pub trait ArchiveStore {
    fn publish(&self, backup: &Completed) -> Result<bool, Error>;
}

pub fn export(
    api: &mut impl Api,
    store: &impl ArchiveStore,
    password: &str,
) -> Result<usize, Error> {
    let response = api.request(json!({"type":"backup/info"}))?;
    let backups = completed(&response)?;
    if backups.is_empty() {
        return Err(Error::Invalid);
    }
    let mut count = 0;
    for backup in backups {
        api.request(json!({"type":"backup/can_decrypt_on_download","backup_id":backup.backup_id,"agent_id":"backup.local","password":password}))?;
        if store.publish(&backup)? {
            count += 1;
        }
    }
    Ok(count)
}

pub fn create_and_wait(api: &mut impl Api) -> Result<(), Error> {
    let job = api.request(json!({"type":"backup/generate_with_automatic_settings"}))?;
    let id = job["backup_job_id"].as_str().ok_or(Error::Api)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        let info = api.request(json!({"type":"backup/info"}))?;
        if info["state"] == "idle" && completed(&info)?.iter().any(|b| b.backup_id == id) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(Error::Api)
}

pub struct LocalArchives {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub uid: u32,
    pub gid: u32,
}

fn open_regular(path: &Path) -> Result<File, Error> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid);
    }
    Ok(file)
}

fn manifest(file: &mut File, complete: bool) -> Result<Value, Error> {
    file.seek(SeekFrom::Start(0))?;
    let mut archive = tar::Archive::new(file);
    let mut metadata = None;
    let mut payload = false;
    for entry in archive.entries_with_seek()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            return Err(Error::Invalid);
        }
        let path = entry.path()?.into_owned();
        if path == Path::new("backup.json") || path == Path::new("./backup.json") {
            if metadata.is_some() || entry.size() > 65536 {
                return Err(Error::Invalid);
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            metadata = Some(serde_json::from_slice::<Value>(&bytes).map_err(|_| Error::Invalid)?);
        } else {
            if path == Path::new("homeassistant.tar.gz")
                || path == Path::new("./homeassistant.tar.gz")
            {
                payload = entry.size() > 0;
            }
            if complete {
                std::io::copy(&mut entry, &mut std::io::sink())?;
            }
            if !complete && payload && metadata.is_some() {
                break;
            }
        }
    }
    let metadata = metadata.ok_or(Error::Invalid)?;
    if !payload
        || metadata["protected"] != true
        || metadata["homeassistant"]["version"].as_str().is_none()
    {
        return Err(Error::Invalid);
    }
    Ok(metadata)
}

fn hash(file: &mut File) -> Result<Vec<u8>, Error> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut bytes = vec![0u8; 65536];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(hash.finalize().to_vec())
}

impl ArchiveStore for LocalArchives {
    fn publish(&self, backup: &Completed) -> Result<bool, Error> {
        let mut source = None;
        for entry in fs::read_dir(&self.source)? {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "tar")
            {
                continue;
            }
            let mut file = open_regular(&entry.path())?;
            // Validate every completed-looking candidate rather than trusting its name.
            let metadata = manifest(&mut file, false)?;
            if metadata["slug"].as_str() == Some(&backup.backup_id) {
                if source.is_some() || file.metadata()?.len() != backup.size {
                    return Err(Error::Invalid);
                }
                source = Some(file);
            }
        }
        let mut source = source.ok_or(Error::Invalid)?;
        manifest(&mut source, true)?;
        let before = source.metadata()?;
        let digest = hash(&mut source)?;
        let target = self
            .destination
            .join(format!("ha-{}.tar", backup.backup_id));
        if fs::symlink_metadata(&target).is_ok() {
            let mut existing = open_regular(&target)?;
            if hash(&mut existing)? != digest {
                return Err(Error::Invalid);
            }
            return Ok(false);
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".syncthing.homeassistant-")
            .suffix(".tmp")
            .tempfile_in(&self.destination)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        nix::unistd::fchown(
            temporary.as_file().as_raw_fd(),
            Some(nix::unistd::Uid::from_raw(self.uid)),
            Some(nix::unistd::Gid::from_raw(self.gid)),
        )
        .map_err(|_| Error::Invalid)?;
        source.seek(SeekFrom::Start(0))?;
        std::io::copy(&mut source, &mut temporary)?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        if hash(temporary.as_file_mut())? != digest
            || source.metadata()?.len() != before.len()
            || source.metadata()?.modified()? != before.modified()?
        {
            return Err(Error::Invalid);
        }
        temporary
            .persist_noclobber(target)
            .map_err(|_| Error::Invalid)?;
        File::open(&self.destination)?.sync_all()?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;

/// Timestamp from completed local consumer metadata; never the export job start time.
pub fn latest_completed(info: &Value) -> Result<u64, Error> {
    let completed = completed(info)?;
    info["backups"]
        .as_array()
        .ok_or(Error::Invalid)?
        .iter()
        .filter(|b| {
            b["backup_id"]
                .as_str()
                .is_some_and(|id| completed.iter().any(|c| c.backup_id == id))
        })
        .map(|b| {
            chrono::DateTime::parse_from_rfc3339(b["date"].as_str().ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)
                .and_then(|d| u64::try_from(d.timestamp()).map_err(|_| Error::Invalid))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .ok_or(Error::Invalid)
}
