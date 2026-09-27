use super::{butane_root, vm_name, SecretDeliverArgs, VmProvisionArgs};
use anyhow::{anyhow, Context, Result};
use keepass::{Database, DatabaseKey};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::Path,
    process::{Command, Stdio},
};

const INSTALL: &str = r#"set -euo pipefail
umask 077
dir=/etc/credstore.encrypted/skillet
install -d -m 0700 "$dir"
tmp=$(mktemp "$dir/.pihole_web_password.XXXXXX")
trap "rm -f -- $tmp" EXIT
systemd-creds encrypt --with-key=host --name=pihole_web_password - - > "$tmp"
systemd-creds decrypt --name=pihole_web_password "$tmp" - >/dev/null
chmod 0600 "$tmp"
mv -f -- "$tmp" "$dir/pihole_web_password.cred"
systemctl start --wait skillet-full-apply.service"#;

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    let path = format!("skillet/hosts/{}/pihole/web-password", args.hostname);
    let database = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let secret = read_vault(&database, args.key_file.as_deref(), &path)?;
    if !args.identity.is_file() || !args.known_hosts.is_file() {
        return Err(anyhow!(
            "SSH identity and recorded known-hosts file must exist"
        ));
    }
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
    install(&mut command, &secret)
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
                    "sudo -n systemctl start --wait skillet-full-apply.service",
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

fn read_vault(path: &Path, key_file: Option<&Path>, entry_path: &str) -> Result<String> {
    // Check file availability before prompting for the unlock password.
    let mut database_file = fs::File::open(path).with_context(|| format!(
        "KeePassXC database is missing or unreadable at {}; place the synced vault there or pass --database",
        path.display()
    ))?;
    let mut key_file = key_file
        .map(fs::File::open)
        .transpose()
        .context("KeePassXC key file is missing or unreadable")?;
    let password = rpassword::prompt_password("KeePassXC database password: ")
        .context("reading database password from terminal")?;
    let mut key = DatabaseKey::new().with_password(&password);
    if let Some(file) = key_file.as_mut() {
        key = key.with_keyfile(file)?;
    }
    let database = Database::open(&mut database_file, key)
        .context("opening KeePassXC database; check password and key file")?;
    lookup(&database, entry_path)
}

fn lookup(database: &Database, path: &str) -> Result<String> {
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
    if matches.len() != 1 {
        return Err(anyhow!("KeePassXC entry {path} must exist exactly once"));
    }
    let entry = matches.remove(0);
    let secret = entry
        .get_password()
        .ok_or_else(|| anyhow!("KeePassXC entry {path} has no Password field"))?;
    if secret.is_empty() {
        return Err(anyhow!(
            "KeePassXC entry {path} has an empty Password field"
        ));
    }
    Ok(secret.to_owned())
}

fn install(command: &mut Command, secret: &str) -> Result<()> {
    if secret.is_empty() {
        return Err(anyhow!("refusing to deliver an empty credential"));
    }
    // The script is constant and contains no user input. Secret bytes travel
    // only over stdin, never through command arguments or diagnostics.
    let remote = format!("sudo -n bash -c '{INSTALL}'");
    let mut child = command
        .arg(remote)
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
