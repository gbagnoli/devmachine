use super::{butane_root, vm_name, SecretDeliverArgs, VmProvisionArgs};
use anyhow::{anyhow, Context, Result};
use keepass::{Database, DatabaseKey};
use std::{
    fs,
    io::{Read as _, Write as _},
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

mod vault_cache;

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    let path = format!("skillet/hosts/{}/pihole/web-password", args.hostname);
    let database = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    if !args.identity.is_file() || !args.known_hosts.is_file() {
        return Err(anyhow!(
            "SSH identity and recorded known-hosts file must exist"
        ));
    }
    let mut vault = open_vault(&database, args.key_file.as_deref())?;
    let secret = if let Some(secret) = lookup(&vault.database, &path)? {
        if fs::read(&vault.path).context("rechecking vault before delivery")? != vault.original {
            return Err(anyhow!(
                "KeePassXC database changed while Skillet was running; reopen it and retry"
            ));
        }
        secret
    } else {
        if remote_credential_state(args)? != RemoteCredentialState::Absent {
            return Err(anyhow!(
                "host already has a Pi-hole credential; restore the missing KeePassXC entry instead of creating a replacement"
            ));
        }
        let secret = random_password()?;
        create_entry(&mut vault.database, &path, &secret)?;
        save_vault(&vault, args.key_file.as_deref(), &path, &secret)?;
        secret
    };
    let mut command = ssh_command(args);
    install(&mut command, &secret)
}

pub(super) fn lock_vault(path: Option<&Path>) -> Result<()> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_database_path()?,
    };
    // A symlink at the default XDG location points to the same cache entry as
    // an explicit path to its target.
    let canonical = canonical_database(&path)?;
    vault_cache::clear(&canonical)?;
    println!("Vault unlock removed from the kernel keyring");
    Ok(())
}

fn ssh_command(args: &SecretDeliverArgs) -> Command {
    let mut command = Command::new("ssh");
    command
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
        ])
        .arg(format!("UserKnownHostsFile={}", args.known_hosts.display()))
        .arg("-i")
        .arg(&args.identity)
        .arg("-p")
        .arg(args.port.to_string())
        .arg(&args.target);
    command
}

#[derive(PartialEq, Eq)]
enum RemoteCredentialState {
    Present,
    Absent,
}

fn remote_credential_state(args: &SecretDeliverArgs) -> Result<RemoteCredentialState> {
    let output = ssh_command(args)
        .arg("sudo -n /var/usrlocal/bin/skillet-clamps credential state pihole_web_password")
        .output()
        .context("checking host state before generating a production credential")?;
    if !output.status.success() {
        return Err(anyhow!(
            "SSH host credential state check failed; no production credential was generated"
        ));
    }
    match output.stdout.as_slice() {
        b"present\n" => Ok(RemoteCredentialState::Present),
        b"absent\n" => Ok(RemoteCredentialState::Absent),
        _ => Err(anyhow!("host returned an unrecognized credential state")),
    }
}

fn default_database_path() -> Result<std::path::PathBuf> {
    let xdg = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    database_path_from(xdg.as_deref(), home.as_deref())
}

fn database_path_from(
    xdg_data_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<std::path::PathBuf> {
    let data_home = if let Some(path) = xdg_data_home.filter(|path| path.is_absolute()) {
        path.to_path_buf()
    } else {
        let home = home
            .filter(|path| path.is_absolute())
            .ok_or_else(|| anyhow!("set an absolute XDG_DATA_HOME or HOME, or pass --database"))?;
        home.join(".local/share")
    };
    Ok(data_home.join("skillet/secrets.kdbx"))
}

pub(super) fn provision_vm(args: &VmProvisionArgs) -> Result<()> {
    if args.hostname != "clamps" {
        return Err(anyhow!(
            "application provisioning currently supports clamps only"
        ));
    }
    let name = vm_name(&args.hostname, &args.instance)?;
    let butane = butane_root()?;
    let helper = butane.join("bin/test-vm");
    let status = Command::new(&helper)
        .args([&args.hostname, "status", &args.instance])
        .stdout(Stdio::null())
        .status()
        .context("validating owned VM")?;
    if !status.success() {
        return Err(anyhow!("VM ownership or status validation failed"));
    }
    let run_dir = butane.join("runs").join(name);
    let identity = run_dir.join("ssh/id_ed25519");
    let known_hosts = run_dir.join("ssh/known_hosts");
    let port = read_vm_port(&run_dir.join("run.conf"))?;
    let mut command = Command::new("ssh");
    command
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
        ])
        .arg(format!("UserKnownHostsFile={}", known_hosts.display()))
        .arg("-i")
        .arg(&identity)
        .arg("-p")
        .arg(port.to_string())
        .arg("giacomo@127.0.0.1");
    if !args.rotate {
        // An existing encrypted credential is reusable after reboot or on a
        // different workstation. Missing or corrupt files fail in full apply.
        let check = Command::new("ssh")
            .args([
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
            ])
            .arg(format!("UserKnownHostsFile={}", known_hosts.display()))
            .arg("-i")
            .arg(&identity)
            .arg("-p")
            .arg(port.to_string())
            .args([
                "giacomo@127.0.0.1",
                "sudo -n test -s /etc/credstore.encrypted/skillet/pihole_web_password.cred",
            ])
            .status()
            .context("checking VM credential")?;
        if check.success() {
            let apply = Command::new("ssh")
                .args([
                    "-T",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "IdentitiesOnly=yes",
                    "-o",
                    "StrictHostKeyChecking=yes",
                    "-o",
                ])
                .arg(format!("UserKnownHostsFile={}", known_hosts.display()))
                .arg("-i")
                .arg(&identity)
                .arg("-p")
                .arg(port.to_string())
                .args([
                    "giacomo@127.0.0.1",
                    "sudo -n systemctl start skillet-full-apply.service",
                ])
                .status()
                .context("running full apply")?;
            if !apply.success() {
                return Err(anyhow!("full apply failed"));
            }
            return Ok(());
        }
    }
    let mut bytes = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let secret = hex::encode(bytes);
    install(&mut command, &secret)
}

fn read_vm_port(manifest: &Path) -> Result<u16> {
    let contents = fs::read_to_string(manifest)?;
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("ssh_port="))
        .ok_or_else(|| anyhow!("VM manifest has no SSH port"))?;
    let port: u16 = value.parse().context("invalid VM SSH port")?;
    if !(2200..=2299).contains(&port) {
        return Err(anyhow!("VM SSH port is outside the test range"));
    }
    Ok(port)
}

struct OpenVault {
    path: PathBuf,
    original: Vec<u8>,
    database: Database,
    password: String,
}

fn canonical_database(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!(
        "KeePassXC database is missing or unreadable at {}; place the synced vault there or pass --database",
        path.display()
    ))
}

fn database_key(password: &str, key_file: Option<&Path>) -> Result<DatabaseKey> {
    let mut key = DatabaseKey::new().with_password(password);
    if let Some(path) = key_file {
        let mut file =
            fs::File::open(path).context("KeePassXC key file is missing or unreadable")?;
        key = key.with_keyfile(&mut file)?;
    }
    Ok(key)
}

fn open_with_password(bytes: &[u8], password: &str, key_file: Option<&Path>) -> Result<Database> {
    let mut reader = bytes;
    Database::open(&mut reader, database_key(password, key_file)?)
        .context("opening KeePassXC database; check password and key file")
}

fn open_vault(path: &Path, key_file: Option<&Path>) -> Result<OpenVault> {
    // Resolve the symlink before any eventual atomic replacement, so the
    // Syncthing-managed target is changed instead of replacing the symlink.
    let path = canonical_database(path)?;
    let original = fs::read(&path).context("reading KeePassXC database")?;
    if let Some(cached) = vault_cache::read(&path)? {
        if let Ok(database) = open_with_password(&original, &cached, key_file) {
            return Ok(OpenVault {
                path,
                original,
                database,
                password: cached,
            });
        }
        vault_cache::clear(&path)?;
    }
    let password = rpassword::prompt_password("KeePassXC database password: ")
        .context("reading database password from terminal")?;
    let database = open_with_password(&original, &password, key_file)?;
    vault_cache::store(&path, &password)?;
    Ok(OpenVault {
        path,
        original,
        database,
        password,
    })
}

fn lookup(database: &Database, path: &str) -> Result<Option<String>> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(anyhow!("invalid KeePassXC entry path"));
    }
    let mut groups = vec![database.root().id()];
    for part in &parts[..parts.len() - 1] {
        groups = groups
            .into_iter()
            .filter_map(|id| database.group(id))
            .flat_map(|group| group.group_ids().collect::<Vec<_>>())
            .filter(|id| database.group(*id).is_some_and(|group| group.name == *part))
            .collect();
    }
    let mut matches = groups
        .iter()
        .filter_map(|id| database.group(*id))
        .flat_map(|group| group.entry_ids().collect::<Vec<_>>())
        .filter_map(|id| database.entry(id))
        .filter(|entry| entry.get_title() == parts.last().copied())
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(anyhow!("KeePassXC entry {path} is ambiguous"));
    }
    let Some(entry) = matches.pop() else {
        return Ok(None);
    };
    let secret = entry
        .get_password()
        .ok_or_else(|| anyhow!("KeePassXC entry {path} has no Password field"))?;
    if secret.is_empty() {
        return Err(anyhow!(
            "KeePassXC entry {path} has an empty Password field"
        ));
    }
    Ok(Some(secret.to_owned()))
}

fn random_password() -> Result<String> {
    let mut bytes = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex::encode(bytes))
}

fn create_entry(database: &mut Database, path: &str, password: &str) -> Result<()> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(anyhow!("invalid KeePassXC entry path"));
    }
    let mut parent_id = database.root().id();
    for name in &parts[..parts.len() - 1] {
        let parent = database
            .group(parent_id)
            .ok_or_else(|| anyhow!("vault group disappeared"))?;
        let matching = parent
            .group_ids()
            .filter(|id| database.group(*id).is_some_and(|group| group.name == *name))
            .collect::<Vec<_>>();
        parent_id = match matching.as_slice() {
            [id] => *id,
            [] => {
                let mut parent = database
                    .group_mut(parent_id)
                    .ok_or_else(|| anyhow!("vault group disappeared"))?;
                let mut group = parent.add_group();
                (*name).clone_into(&mut group.name);
                group.id()
            }
            _ => return Err(anyhow!("KeePassXC group {name} is ambiguous")),
        };
    }
    if lookup(database, path)?.is_some() {
        return Err(anyhow!("KeePassXC entry {path} already exists"));
    }
    let mut parent = database
        .group_mut(parent_id)
        .ok_or_else(|| anyhow!("vault group disappeared"))?;
    let mut entry = parent.add_entry();
    entry.set_unprotected("Title", parts[parts.len() - 1]);
    entry.set_protected("Password", password);
    Ok(())
}

fn save_vault(
    vault: &OpenVault,
    key_file: Option<&Path>,
    entry_path: &str,
    secret: &str,
) -> Result<()> {
    let parent = vault
        .path
        .parent()
        .ok_or_else(|| anyhow!("vault has no parent directory"))?;
    let lock_path = parent.join(".skillet-vault.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)
        .context("opening vault write lock")?;
    lock.lock().context("locking vault for update")?;
    if fs::read(&vault.path).context("rechecking vault before write")? != vault.original {
        return Err(anyhow!(
            "KeePassXC database changed while Skillet was running; reopen it and retry"
        ));
    }
    let mut candidate =
        tempfile::NamedTempFile::new_in(parent).context("creating encrypted vault update")?;
    vault
        .database
        .save(
            candidate.as_file_mut(),
            database_key(&vault.password, key_file)?,
        )
        .context("saving KeePassXC database")?;
    candidate.as_file_mut().flush()?;
    candidate
        .as_file()
        .set_permissions(fs::metadata(&vault.path)?.permissions())?;
    candidate.as_file().sync_all()?;
    let candidate_bytes = fs::read(candidate.path())?;
    let reopened = open_with_password(&candidate_bytes, &vault.password, key_file)?;
    if lookup(&reopened, entry_path)?.as_deref() != Some(secret) {
        return Err(anyhow!(
            "saved vault did not retain the generated credential"
        ));
    }
    // Retain a separate encrypted copy of the previous valid vault before
    // replacing it. KeePassXC/Syncthing can then recover from a bad write.
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let backup_name = format!(
        "{}.skillet-{stamp}.bak",
        vault
            .path
            .file_name()
            .ok_or_else(|| anyhow!("vault has no filename"))?
            .to_string_lossy()
    );
    let mut backup = tempfile::NamedTempFile::new_in(parent).context("creating vault backup")?;
    backup.write_all(&vault.original)?;
    backup
        .as_file()
        .set_permissions(fs::metadata(&vault.path)?.permissions())?;
    backup.as_file().sync_all()?;
    backup
        .persist_noclobber(parent.join(backup_name))
        .context("preserving previous encrypted vault")?;
    if fs::read(&vault.path).context("rechecking vault before replacement")? != vault.original {
        return Err(anyhow!(
            "KeePassXC database changed during save; generated value was not installed"
        ));
    }
    candidate
        .persist(&vault.path)
        .context("atomically replacing KeePassXC database")?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn install(command: &mut Command, secret: &str) -> Result<()> {
    if secret.is_empty() {
        return Err(anyhow!("refusing to deliver an empty credential"));
    }
    let mut child = command
        .arg("sudo -n /var/usrlocal/bin/skillet-clamps credential install pihole_web_password skillet-full-apply.service")
        .stdin(Stdio::piped())
        .spawn()
        .context("opening SSH credential delivery")?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("SSH stdin unavailable"))?
        .write_all(secret.as_bytes())
        .context("sending credential to SSH")?;
    let status = child.wait().context("waiting for credential delivery")?;
    if !status.success() {
        return Err(anyhow!(
            "credential delivery or full apply failed: {status}"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "secret_delivery/tests.rs"]
mod tests;
