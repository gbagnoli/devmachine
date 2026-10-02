use super::{
    butane_root, tailscale, vm_name, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs,
    VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use keepass::{Database, DatabaseKey};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read as _, Write as _},
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

mod vault_cache;

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    let ui_config = if args.service == "caddy" {
        Some(
            skillet_cli_common::hosts::ui_config_for_host(&args.hostname)
                .ok_or_else(|| anyhow!("host {} has no declared UI services", args.hostname))?,
        )
    } else {
        None
    };
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
    match args.service.as_str() {
        "pihole" => {
            let declared = skillet_cli_common::hosts::ui_config_for_host(&args.hostname)
                .is_some_and(|config| {
                    config
                        .services
                        .iter()
                        .any(|service| service.name == "pihole")
                });
            if !declared {
                return Err(anyhow!(
                    "host {} does not declare a Pi-hole UI",
                    args.hostname
                ));
            }
            let path = format!("skillet/hosts/{}/pihole/web-password", args.hostname);
            let secret = if let Some(secret) = lookup(&vault.database, &path)? {
                ensure_vault_unchanged(&vault)?;
                secret
            } else {
                if remote_credential_state(args)? != RemoteCredentialState::Absent {
                    return Err(anyhow!(
                        "host already has a Pi-hole credential; restore the missing KeePassXC entry instead of creating a replacement"
                    ));
                }
                let secret = random_password()?;
                create_entry(&mut vault.database, &path, &secret)?;
                save_vault(&mut vault, args.key_file.as_deref(), &path, &secret)?;
                secret
            };
            let mut command = ssh_command(args);
            install(&mut command, &args.hostname, "pihole_web_password", &secret)
        }
        "tailscale" => {
            if args.hostname != "clamps" {
                return Err(anyhow!(
                    "host {} does not declare Tailscale credential delivery",
                    args.hostname
                ));
            }
            let credentials = tailscale_credentials(&vault)?;
            let auth_key = tailscale::create_auth_key(
                &credentials,
                tailscale::SERVER_TAG,
                "Skillet clamps production host",
            )?;
            let mut command = ssh_command(args);
            install(
                &mut command,
                &args.hostname,
                "tailscale_auth_key",
                &auth_key.key,
            )
        }
        "caddy" => deliver_caddy_from_vault(args, &mut vault, ui_config.as_ref()),
        _ => Err(anyhow!("unsupported secret service {}", args.service)),
    }
}

fn deliver_caddy_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut OpenVault,
    ui_config: Option<&skillet_cli_common::hosts::HostUiConfig>,
) -> Result<()> {
    let ui_config = ui_config.ok_or_else(|| anyhow!("missing host UI declaration"))?;
    let environment = args.environment.as_str();
    let domain_path = format!("skillet/environments/{environment}/dns/ui-domain");
    let domain_prefix = lookup(&vault.database, &domain_path)?;
    let zone_path = format!("skillet/environments/{environment}/dns/cloudflare-zone-id");
    let zone_id = lookup(&vault.database, &zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let zone_id = zone_id.trim();
    crate::cloudflare::validate_zone_id(zone_id)?;
    let creator =
        lookup(&vault.database, "skillet/cloudflare/token-creator")?.ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let cloudflare = crate::cloudflare::Cloudflare::new();
    let zone = cloudflare.zone(&creator, zone_id)?;
    let domain = skillet_caddy::resolve_ui_domain(&zone.name, domain_prefix.as_deref())
        .context("resolving KeePassXC relative UI domain beneath its Cloudflare zone")?;
    let sites = skillet_caddy::CaddySites::from_host(
        &args.hostname,
        &skillet_caddy::UiEnvironment {
            ui_domain: domain.clone(),
            acme_staging: args.environment.acme_staging(),
        },
        &ui_config.services,
    )?;
    let tailnet = tailscale_credentials(vault)?;
    let device =
        tailscale::find_device_by_hostname(&tailnet, &args.hostname, tailscale::SERVER_TAG)?;
    let dns_marker = format!("skillet:{environment}:{}", args.hostname);
    let dns =
        crate::cloudflare::desired_records(&sites.machine_hostname, &device.addresses, &sites)?;
    ensure_vault_unchanged(vault)?;
    let token = host_acme_token(args, vault, &cloudflare, &creator, zone_id)?;
    cloudflare.zone(&token, zone_id)?;
    ensure_vault_unchanged(vault)?;
    cloudflare.reconcile_dns(&token, zone_id, &dns_marker, &domain, &dns)?;
    ensure_vault_unchanged(vault)?;
    let sites = serde_json::to_string(&sites)?;
    for (credential, value) in [
        ("caddy_sites", sites.as_str()),
        ("cloudflare_acme_token", token.as_str()),
    ] {
        install_deferred_for_unit(
            &mut ssh_command(args),
            &args.hostname,
            credential,
            "skillet-caddy-apply.service",
            value,
        )?;
    }
    let status = ssh_command(args)
        .arg("sudo -n systemctl start skillet-caddy-apply.service")
        .status()
        .context("starting Caddy apply after both credentials were delivered")?;
    if !status.success() {
        return Err(anyhow!("Caddy apply failed with status {status}"));
    }
    Ok(())
}

fn host_acme_token(
    args: &SecretDeliverArgs,
    vault: &mut OpenVault,
    api: &crate::cloudflare::Cloudflare,
    creator: &str,
    zone_id: &str,
) -> Result<String> {
    let environment = args.environment.as_str();
    let token_path = format!(
        "skillet/environments/{environment}/hosts/{}/cloudflare/acme-token",
        args.hostname
    );
    if let Some(token) = lookup(&vault.database, &token_path)? {
        return Ok(token);
    }
    let legacy = if args.environment == UiEnvironmentName::Production {
        lookup(
            &vault.database,
            &format!("skillet/hosts/{}/cloudflare/acme-token", args.hostname),
        )?
    } else {
        None
    };
    if let Some(token) = legacy {
        create_entry(&mut vault.database, &token_path, &token)?;
        save_vault(vault, args.key_file.as_deref(), &token_path, &token)?;
        return Ok(token);
    }
    let token_name = format!("skillet:{environment}:{}", args.hostname);
    // A previous request may have reached Cloudflare before its response was
    // lost. With no vault credential to reuse, clean up only child tokens
    // carrying this exact Skillet-owned name.
    for orphan in api.token_ids_by_name(creator, &token_name)? {
        api.revoke_token(creator, &orphan)?;
    }
    let issued = api.create_zone_token(creator, zone_id, &token_name, None)?;
    if let Err(error) = create_entry(&mut vault.database, &token_path, &issued.value)
        .and_then(|()| save_vault(vault, args.key_file.as_deref(), &token_path, &issued.value))
    {
        if let Err(revoke_error) = api.revoke_token(creator, &issued.id) {
            return Err(anyhow!("saving issued Cloudflare credential failed ({error}); revoking token {} also failed ({revoke_error})", issued.id));
        }
        return Err(
            error.context("saving new Cloudflare token into KeePassXC; issued token was revoked")
        );
    }
    Ok(issued.value)
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

fn ensure_vault_unchanged(vault: &OpenVault) -> Result<()> {
    if fs::read(&vault.path).context("rechecking vault before delivery")? != vault.original {
        return Err(anyhow!(
            "KeePassXC database changed while Skillet was running; reopen it and retry"
        ));
    }
    Ok(())
}

fn tailscale_credentials(vault: &OpenVault) -> Result<tailscale::OAuthCredentials> {
    let client_id = lookup(&vault.database, "skillet/tailscale/provisioner-client-id")?
        .ok_or_else(|| {
            anyhow!("KeePassXC entry skillet/tailscale/provisioner-client-id is missing")
        })?;
    let client_secret = lookup(
        &vault.database,
        "skillet/tailscale/provisioner-client-secret",
    )?
    .ok_or_else(|| {
        anyhow!("KeePassXC entry skillet/tailscale/provisioner-client-secret is missing")
    })?;
    tailscale::OAuthCredentials::new(client_id, client_secret)
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
    let run_dir = butane.join("runs").join(&name);
    let identity = run_dir.join("ssh/id_ed25519");
    let known_hosts = run_dir.join("ssh/known_hosts");
    let port = read_vm_port(&run_dir.join("run.conf"))?;
    let mut ssh = VmSsh::new(&identity, &known_hosts, port);
    validate_vm_tailscale_delivery(&mut ssh)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = open_vault(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    let expected_hostname = name.as_str();

    write_pending_tailscale(&run_dir, expected_hostname)?;
    let mut addresses = vm_tailscale_addresses(&mut ssh).unwrap_or_default();
    if addresses.is_empty() {
        let auth_key = tailscale::create_auth_key(
            &credentials,
            tailscale::SMOKE_TAG,
            &format!("Skillet disposable VM {expected_hostname}"),
        )?;
        ssh.install("tailscale_auth_key", &auth_key.key)?;
    }

    let pihole_credential = "/etc/credstore.encrypted/skillet/pihole_web_password.cred";
    let has_pihole_credential = vm_credential_present(&mut ssh, pihole_credential)?;
    if args.rotate || !has_pihole_credential {
        let secret = random_password()?;
        ssh.install("pihole_web_password", &secret)?;
    } else {
        ssh.run("sudo -n systemctl start skillet-full-apply.service")?;
    }

    addresses = wait_for_vm_tailscale(&mut ssh)?;
    let record = tailscale::find_device(
        &credentials,
        expected_hostname,
        tailscale::SMOKE_TAG,
        &addresses,
    )?;
    save_vm_tailscale_record(&run_dir, &record)?;
    if args.with_ui {
        provision_vm_ui(args, &run_dir, &mut ssh, &mut vault, &record)?;
    }
    remove_pending_tailscale(&run_dir)?;
    println!("Tailscale connected VM {expected_hostname}");
    Ok(())
}

pub(super) fn remove_vm_from_tailscale(args: &VmDestroyArgs) -> Result<()> {
    if args.hostname != "clamps" {
        return Ok(());
    }
    let name = vm_name(&args.hostname, &args.instance)?;
    let butane = butane_root()?;
    let run_dir = butane.join("runs").join(&name);
    let pending = run_dir.join("tailscale-pending");
    let record_path = run_dir.join("tailscale.json");
    let cloudflare_path = run_dir.join("cloudflare.json");
    if !pending.exists() && !record_path.exists() && !cloudflare_path.exists() {
        return Ok(());
    }
    if cloudflare_path.exists() {
        cleanup_vm_cloudflare(args, &cloudflare_path)?;
    }
    if !pending.exists() && !record_path.exists() {
        return Ok(());
    }
    let expected = if record_path.exists() {
        Some(read_vm_tailscale_record(&record_path)?)
    } else {
        None
    };
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let vault = open_vault(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    if tailscale::remove_device_for_hostname(
        &credentials,
        &name,
        tailscale::SMOKE_TAG,
        expected.as_ref(),
    )?
    .is_some()
    {
        println!("Removed Tailscale device for {name}");
    }
    remove_pending_tailscale(&run_dir)?;
    if record_path.exists() {
        fs::remove_file(record_path).context("removing Tailscale VM metadata")?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloudflareVmOwnership {
    environment: String,
    marker: String,
    zone_id: String,
    ui_domain: String,
    token_name: String,
    token_id: Option<String>,
    expires_on: Option<String>,
    record_ids: Vec<String>,
}

fn provision_vm_ui(
    args: &VmProvisionArgs,
    run_dir: &Path,
    ssh: &mut VmSsh<'_>,
    vault: &mut OpenVault,
    device: &tailscale::DeviceRecord,
) -> Result<()> {
    let environment = UiEnvironmentName::Test.as_str();
    let domain_path = format!("skillet/environments/{environment}/dns/ui-domain");
    let zone_path = format!("skillet/environments/{environment}/dns/cloudflare-zone-id");
    let domain_prefix = lookup(&vault.database, &domain_path)?;
    let zone_id = lookup(&vault.database, &zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let creator =
        lookup(&vault.database, "skillet/cloudflare/token-creator")?.ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let zone_id = zone_id.trim().to_string();
    crate::cloudflare::validate_zone_id(&zone_id)?;
    let api = crate::cloudflare::Cloudflare::new();
    let zone = api.zone(&creator, &zone_id)?;
    let ui_domain = skillet_caddy::resolve_ui_domain(&zone.name, domain_prefix.as_deref())
        .context("resolving test relative UI domain beneath its Cloudflare zone")?;
    let host_ui = skillet_cli_common::hosts::ui_config_for_host(&args.hostname)
        .ok_or_else(|| anyhow!("host {} has no declared UI services", args.hostname))?;
    let sites = skillet_caddy::CaddySites::from_host(
        &args.hostname,
        &skillet_caddy::UiEnvironment {
            ui_domain: ui_domain.clone(),
            acme_staging: true,
        },
        &host_ui.services,
    )?;
    let dns =
        crate::cloudflare::desired_records(&sites.machine_hostname, &device.addresses, &sites)?;
    let marker = format!("skillet:test:{}:{}", args.hostname, args.instance);
    let token_name = format!("skillet:test:{}-{}", args.hostname, args.instance);
    let metadata_path = run_dir.join("cloudflare.json");
    let mut ownership = CloudflareVmOwnership {
        environment: environment.to_string(),
        marker,
        zone_id,
        ui_domain,
        token_name,
        token_id: None,
        expires_on: None,
        record_ids: Vec::new(),
    };
    write_cloudflare_ownership(&metadata_path, &ownership)?;
    ensure_vault_unchanged(vault)?;
    let issued = api.create_zone_token(
        &creator,
        &ownership.zone_id,
        &ownership.token_name,
        Some(std::time::Duration::from_hours(12)),
    )?;
    ownership.token_id = Some(issued.id.clone());
    ownership.expires_on.clone_from(&issued.expires_on);
    write_cloudflare_ownership(&metadata_path, &ownership)?;
    let work = (|| {
        let owned = api.reconcile_dns(
            &issued.value,
            &ownership.zone_id,
            &ownership.marker,
            &ownership.ui_domain,
            &dns,
        )?;
        ownership.record_ids = owned.records.into_iter().map(|record| record.id).collect();
        write_cloudflare_ownership(&metadata_path, &ownership)?;
        ssh.install_deferred(
            "caddy_sites",
            "skillet-caddy-apply.service",
            &serde_json::to_string(&sites)?,
        )?;
        ssh.install_deferred(
            "cloudflare_acme_token",
            "skillet-caddy-apply.service",
            &issued.value,
        )?;
        ssh.run("sudo -n systemctl start skillet-caddy-apply.service")?;
        for old_id in api.token_ids_by_name(&creator, &ownership.token_name)? {
            if old_id != issued.id {
                api.revoke_token(&creator, &old_id)?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })();
    if let Err(error) = work {
        return Err(error.context("provisioning disposable Cloudflare DNS and Caddy"));
    }
    Ok(())
}

fn cleanup_vm_cloudflare(args: &VmDestroyArgs, metadata_path: &Path) -> Result<()> {
    let bytes = fs::read(metadata_path).context("reading Cloudflare VM ownership metadata")?;
    let ownership: CloudflareVmOwnership =
        serde_json::from_slice(&bytes).context("decoding Cloudflare VM ownership metadata")?;
    if ownership.environment != UiEnvironmentName::Test.as_str() {
        return Err(anyhow!(
            "refusing VM cleanup for a non-test Cloudflare environment"
        ));
    }
    let expected_marker = format!("skillet:test:{}:{}", args.hostname, args.instance);
    let expected_token_name = format!("skillet:test:{}-{}", args.hostname, args.instance);
    if ownership.marker != expected_marker || ownership.token_name != expected_token_name {
        return Err(anyhow!(
            "Cloudflare metadata does not match this VM identity; refusing cleanup"
        ));
    }
    crate::cloudflare::validate_zone_id(&ownership.zone_id)?;
    skillet_caddy::validate_domain_in_zone(&ownership.ui_domain, &ownership.ui_domain)
        .context("validating Cloudflare UI domain in VM metadata")?;
    let vault_path = args
        .database
        .clone()
        .map_or_else(default_database_path, Ok)?;
    let vault = open_vault(&vault_path, args.key_file.as_deref())?;
    let creator =
        lookup(&vault.database, "skillet/cloudflare/token-creator")?.ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let configured_zone = lookup(
        &vault.database,
        "skillet/environments/test/dns/cloudflare-zone-id",
    )?
    .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: skillet/environments/test/dns/cloudflare-zone-id"))?;
    let configured_prefix = lookup(&vault.database, "skillet/environments/test/dns/ui-domain")?;
    if configured_zone.trim() != ownership.zone_id {
        return Err(anyhow!("test Cloudflare configuration differs from the recorded VM owner; restore the original vault values before cleanup"));
    }
    let api = crate::cloudflare::Cloudflare::new();
    let zone = api.zone(&creator, &ownership.zone_id)?;
    let configured_domain =
        skillet_caddy::resolve_ui_domain(&zone.name, configured_prefix.as_deref())
            .context("resolving test UI namespace before VM cleanup")?;
    if configured_domain != ownership.ui_domain {
        return Err(anyhow!("test Cloudflare configuration differs from the recorded VM owner; restore the original relative prefix before cleanup"));
    }
    skillet_caddy::validate_domain_in_zone(&ownership.ui_domain, &zone.name)
        .context("validating recorded test UI domain against its Cloudflare zone")?;
    let cleanup_name = format!("{}:cleanup", ownership.token_name);
    let cleanup_token = api.create_zone_token(
        &creator,
        &ownership.zone_id,
        &cleanup_name,
        Some(std::time::Duration::from_mins(15)),
    )?;
    let cleanup = (|| {
        api.zone(&cleanup_token.value, &ownership.zone_id)?;
        api.remove_dns_marker(
            &cleanup_token.value,
            &ownership.zone_id,
            &ownership.marker,
            &ownership.ui_domain,
        )?;
        for id in api.token_ids_by_name(&creator, &ownership.token_name)? {
            api.revoke_token(&creator, &id)?;
        }
        Ok::<(), anyhow::Error>(())
    })();
    let revoke_cleanup = api
        .token_ids_by_name(&creator, &cleanup_name)
        .and_then(|ids| {
            for id in ids {
                api.revoke_token(&creator, &id)?;
            }
            Ok(())
        });
    cleanup?;
    revoke_cleanup.context("revoking temporary Cloudflare cleanup token")?;
    fs::remove_file(metadata_path).context("removing Cloudflare VM ownership metadata")?;
    Ok(())
}

fn write_cloudflare_ownership(path: &Path, ownership: &CloudflareVmOwnership) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Cloudflare metadata has no parent directory"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .context("creating Cloudflare ownership metadata")?;
    file.as_file_mut()
        .write_all(&serde_json::to_vec(ownership)?)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    file.as_file().sync_all()?;
    file.persist(path)
        .context("recording Cloudflare ownership metadata")?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

struct VmSsh<'a> {
    identity: &'a Path,
    known_hosts: &'a Path,
    port: u16,
}

impl VmSsh<'_> {
    fn command(&self, remote: &str) -> Command {
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
            .arg(format!("UserKnownHostsFile={}", self.known_hosts.display()))
            .arg("-i")
            .arg(self.identity)
            .arg("-p")
            .arg(self.port.to_string())
            .arg("giacomo@127.0.0.1")
            .arg(remote);
        command
    }

    fn run(&mut self, remote: &str) -> Result<()> {
        let output = self
            .command(remote)
            .output()
            .context("running command over VM SSH")?;
        if !output.status.success() {
            return Err(anyhow!("VM command failed with status {}", output.status));
        }
        Ok(())
    }

    fn output(&mut self, remote: &str) -> Result<std::process::Output> {
        self.command(remote)
            .output()
            .context("running command over VM SSH")
    }

    fn install(&mut self, name: &str, secret: &str) -> Result<()> {
        self.install_for_unit(name, "skillet-full-apply.service", secret, false)
    }

    fn install_deferred(&mut self, name: &str, unit: &str, secret: &str) -> Result<()> {
        self.install_for_unit(name, unit, secret, true)
    }

    fn install_for_unit(
        &mut self,
        name: &str,
        unit: &str,
        secret: &str,
        defer_start: bool,
    ) -> Result<()> {
        if secret.is_empty() {
            return Err(anyhow!("refusing to deliver an empty credential"));
        }
        if !unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@'))
        {
            return Err(anyhow!("invalid credential consumer unit"));
        }
        let mut remote =
            format!("sudo -n /var/usrlocal/bin/skillet-clamps credential install {name} {unit}");
        if defer_start {
            remote.push_str(" --no-start");
        }
        let mut child = self
            .command(&remote)
            .stdin(Stdio::piped())
            .spawn()
            .context("opening VM credential delivery")?;
        child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("VM SSH stdin unavailable"))?
            .write_all(secret.as_bytes())
            .context("sending credential to VM over SSH")?;
        let status = child.wait().context("waiting for VM credential delivery")?;
        if !status.success() {
            return Err(anyhow!(
                "VM credential delivery or full apply failed: {status}"
            ));
        }
        Ok(())
    }
}

impl<'a> VmSsh<'a> {
    fn new(identity: &'a Path, known_hosts: &'a Path, port: u16) -> Self {
        Self {
            identity,
            known_hosts,
            port,
        }
    }
}

fn vm_credential_present(ssh: &mut VmSsh<'_>, path: &str) -> Result<bool> {
    let output = ssh.output(&format!("sudo -n test -s {path}"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(anyhow!("could not inspect VM credential state")),
    }
}

fn vm_tailscale_addresses(ssh: &mut VmSsh<'_>) -> Result<std::collections::BTreeSet<String>> {
    let output = ssh.output("sudo -n podman exec tailscale tailscale status --json")?;
    if !output.status.success() {
        return Ok(std::collections::BTreeSet::new());
    }
    parse_tailscale_addresses(&output.stdout)
}

fn parse_tailscale_addresses(output: &[u8]) -> Result<std::collections::BTreeSet<String>> {
    let status: serde_json::Value =
        serde_json::from_slice(output).context("decoding Tailscale status returned by the VM")?;
    if status
        .get("BackendState")
        .and_then(serde_json::Value::as_str)
        != Some("Running")
    {
        return Ok(std::collections::BTreeSet::new());
    }
    let addresses = status
        .get("Self")
        .and_then(|value| value.get("TailscaleIPs"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("Tailscale status has no self addresses"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|address| address.parse::<std::net::IpAddr>().is_ok())
        .map(ToOwned::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(addresses)
}

fn wait_for_vm_tailscale(ssh: &mut VmSsh<'_>) -> Result<std::collections::BTreeSet<String>> {
    for _ in 0..60 {
        if let Ok(addresses) = vm_tailscale_addresses(ssh) {
            if !addresses.is_empty() {
                return Ok(addresses);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    Err(anyhow!("Tailscale did not connect on the VM within 120 seconds; inspect tailscale.service and its journal"))
}

fn write_pending_tailscale(run_dir: &Path, hostname: &str) -> Result<()> {
    let path = run_dir.join("tailscale-pending");
    if path.exists() || path.is_symlink() {
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new_in(run_dir)?;
    file.as_file_mut().write_all(hostname.as_bytes())?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path)
        .context("recording pending Tailscale cleanup")?;
    Ok(())
}

fn save_vm_tailscale_record(run_dir: &Path, record: &tailscale::DeviceRecord) -> Result<()> {
    let path = run_dir.join("tailscale.json");
    if path.is_symlink() {
        return Err(anyhow!("refusing symlinked Tailscale VM metadata"));
    }
    let mut file = tempfile::NamedTempFile::new_in(run_dir)?;
    serde_json::to_writer(file.as_file_mut(), record)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    file.as_file().sync_all()?;
    file.persist(&path)
        .context("saving Tailscale VM identity")?;
    Ok(())
}

fn read_vm_tailscale_record(path: &Path) -> Result<tailscale::DeviceRecord> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(anyhow!("Tailscale VM metadata is not a regular file"));
    }
    serde_json::from_slice(&fs::read(path)?).context("reading Tailscale VM metadata")
}

fn remove_pending_tailscale(run_dir: &Path) -> Result<()> {
    let pending = run_dir.join("tailscale-pending");
    if pending.exists() {
        fs::remove_file(pending).context("removing pending Tailscale metadata")?;
    }
    Ok(())
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

fn validate_vm_tailscale_delivery(ssh: &mut VmSsh<'_>) -> Result<()> {
    let output = ssh
        .output("sudo -n systemctl cat skillet-full-apply.service")
        .context("checking the VM's full-apply credential configuration")?;
    if !output.status.success() {
        return Err(anyhow!(
            "could not read skillet-full-apply.service from the VM"
        ));
    }
    validate_tailscale_unit_config(&String::from_utf8_lossy(&output.stdout))
}

fn validate_tailscale_unit_config(contents: &str) -> Result<()> {
    if !contents.lines().any(|line| {
        line.trim()
            == "LoadCredentialEncrypted=tailscale_auth_key:/etc/credstore.encrypted/skillet/tailscale_auth_key.cred"
    }) {
        return Err(anyhow!(
            "this VM was created from an older Butane config that does not pass the Tailscale credential to full apply; recreate the smoke VM with the current config, then retry provisioning"
        ));
    }
    Ok(())
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
    vault: &mut OpenVault,
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
    vault.original = fs::read(&vault.path).context("refreshing vault snapshot after save")?;
    Ok(())
}

fn install(command: &mut Command, host: &str, credential: &str, secret: &str) -> Result<()> {
    install_for_unit(
        command,
        host,
        credential,
        "skillet-full-apply.service",
        secret,
    )
}

fn install_for_unit(
    command: &mut Command,
    host: &str,
    credential: &str,
    unit: &str,
    secret: &str,
) -> Result<()> {
    install_for_unit_inner(command, host, credential, unit, secret, false)
}

fn install_deferred_for_unit(
    command: &mut Command,
    host: &str,
    credential: &str,
    unit: &str,
    secret: &str,
) -> Result<()> {
    install_for_unit_inner(command, host, credential, unit, secret, true)
}

fn install_for_unit_inner(
    command: &mut Command,
    host: &str,
    credential: &str,
    unit: &str,
    secret: &str,
    defer_start: bool,
) -> Result<()> {
    if secret.is_empty() {
        return Err(anyhow!("refusing to deliver an empty credential"));
    }
    if !host
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(anyhow!("invalid host identifier"));
    }
    let binary = format!("/var/usrlocal/bin/skillet-{host}");
    let mut remote = format!("sudo -n {binary} credential install {credential} {unit}");
    if defer_start {
        remote.push_str(" --no-start");
    }
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
