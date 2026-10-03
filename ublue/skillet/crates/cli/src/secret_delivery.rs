use super::{
    butane_root, tailscale, vm_name, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs,
    VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use skillet_workstation::vault::Vault;
use std::{
    fs,
    io::{Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    validate_delivery_service(&args.hostname, &args.service)?;
    let ui_config = if args.service == "caddy" {
        skillet_hosts::ui_config_for_host(&args.hostname)
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
    let mut vault = Vault::open(&database, args.key_file.as_deref())?;
    match args.service.as_str() {
        "pihole" => {
            let path = format!("skillet/hosts/{}/pihole/web-password", args.hostname);
            let secret = if let Some(secret) = vault.get(&path)? {
                ensure_vault_unchanged(&vault)?;
                secret
            } else {
                if remote_credential_state(args)? != RemoteCredentialState::Absent {
                    return Err(anyhow!(
                        "host already has a Pi-hole credential; restore the missing KeePassXC entry instead of creating a replacement"
                    ));
                }
                let secret = random_password()?;
                vault.insert(&path, &secret)?;
                vault.save_verified(args.key_file.as_deref(), &path, &secret)?;
                secret
            };
            let transport = credential_transport(args)?;
            skillet_vm::credential::install(
                &transport,
                &args.hostname,
                "pihole_web_password",
                "skillet-full-apply.service",
                skillet_vm::credential::ActivationPolicy::StartConsumer,
                secret.as_bytes(),
            )?;
            Ok(())
        }
        "tailscale" => {
            let credentials = tailscale_credentials(&vault)?;
            let auth_key = tailscale::create_auth_key(
                &credentials,
                tailscale::SERVER_TAG,
                &format!("Skillet {0} production host", args.hostname),
            )?;
            let transport = credential_transport(args)?;
            skillet_vm::credential::install(
                &transport,
                &args.hostname,
                "tailscale_auth_key",
                "skillet-full-apply.service",
                skillet_vm::credential::ActivationPolicy::StartConsumer,
                auth_key.key.as_bytes(),
            )?;
            Ok(())
        }
        "caddy" => deliver_caddy_from_vault(args, &mut vault, ui_config.as_ref()),
        _ => Err(anyhow!("unsupported secret service {}", args.service)),
    }
}

fn validate_delivery_service(hostname: &str, service: &str) -> Result<skillet_hosts::HostProfile> {
    let profile = skillet_hosts::profile_for_name(hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {hostname}"))?;
    match service {
        "pihole" | "tailscale" if !profile.supports_service(service) => Err(anyhow!(
            "host {hostname} does not declare {service} credential delivery"
        )),
        "caddy" if profile.ui_services().is_empty() => {
            Err(anyhow!("host {hostname} declares no UI services"))
        }
        "pihole" | "tailscale" | "caddy" => Ok(profile),
        unsupported => Err(anyhow!("unsupported secret service {unsupported}")),
    }
}

fn deliver_caddy_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    ui_config: Option<&skillet_hosts::HostUiConfig>,
) -> Result<()> {
    let ui_config = ui_config.ok_or_else(|| anyhow!("missing host UI declaration"))?;
    let environment = args.environment.as_str();
    let domain_path = format!("skillet/environments/{environment}/dns/ui-domain");
    let domain_prefix = vault.get(&domain_path)?;
    let zone_path = format!("skillet/environments/{environment}/dns/cloudflare-zone-id");
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let zone_id = zone_id.trim();
    skillet_workstation::cloudflare::validate_zone_id(zone_id)?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let cloudflare = skillet_workstation::cloudflare::Cloudflare::new();
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
    let dns = skillet_workstation::cloudflare::desired_records(
        &sites.machine_hostname,
        &device.addresses,
        &sites,
    )?;
    ensure_vault_unchanged(vault)?;
    let account_id = skillet_workstation::cloudflare::Cloudflare::account_id(&zone)?;
    let token = host_acme_token(args, vault, &cloudflare, &creator, zone_id, account_id)?;
    cloudflare.zone(&token, zone_id)?;
    ensure_vault_unchanged(vault)?;
    cloudflare.reconcile_dns(&token, zone_id, &dns_marker, &domain, &dns)?;
    ensure_vault_unchanged(vault)?;
    let sites = serde_json::to_string(&sites)?;
    for (credential, value) in [
        ("caddy_sites", sites.as_str()),
        ("cloudflare_acme_token", token.as_str()),
    ] {
        skillet_vm::credential::install(
            &credential_transport(args)?,
            &args.hostname,
            credential,
            "skillet-caddy-apply.service",
            skillet_vm::credential::ActivationPolicy::DeferConsumer,
            value.as_bytes(),
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
    vault: &mut Vault,
    api: &skillet_workstation::cloudflare::Cloudflare,
    creator: &str,
    zone_id: &str,
    account_id: &str,
) -> Result<String> {
    let environment = args.environment.as_str();
    let token_path = format!(
        "skillet/environments/{environment}/hosts/{}/cloudflare/acme-token",
        args.hostname
    );
    if let Some(token) = vault.get(&token_path)? {
        return Ok(token);
    }
    let legacy = if args.environment == UiEnvironmentName::Production {
        vault.get(&format!(
            "skillet/hosts/{}/cloudflare/acme-token",
            args.hostname
        ))?
    } else {
        None
    };
    if let Some(token) = legacy {
        vault.insert(&token_path, &token)?;
        vault.save_verified(args.key_file.as_deref(), &token_path, &token)?;
        return Ok(token);
    }
    let token_name = format!("skillet:{environment}:{}", args.hostname);
    let issued = api.replace_named_zone_token(creator, zone_id, account_id, &token_name, None)?;
    if let Err(error) = vault
        .insert(&token_path, &issued.value)
        .and_then(|()| vault.save_verified(args.key_file.as_deref(), &token_path, &issued.value))
    {
        if let Err(revoke_error) = api.revoke_token(creator, account_id, &issued.id) {
            return Err(anyhow!("saving issued Cloudflare credential failed ({error}); revoking token {} also failed ({revoke_error})", issued.id));
        }
        return Err(anyhow::Error::new(error)
            .context("saving new Cloudflare token into KeePassXC; issued token was revoked"));
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
    skillet_workstation::vault::lock(Some(&path))?;
    println!("Vault unlock removed from the kernel keyring");
    Ok(())
}

fn ensure_vault_unchanged(vault: &Vault) -> Result<()> {
    vault.ensure_unchanged()?;
    Ok(())
}

fn tailscale_credentials(vault: &Vault) -> Result<tailscale::OAuthCredentials> {
    let client_id = vault
        .get("skillet/tailscale/provisioner-client-id")?
        .ok_or_else(|| {
            anyhow!("KeePassXC entry skillet/tailscale/provisioner-client-id is missing")
        })?;
    let client_secret = vault
        .get("skillet/tailscale/provisioner-client-secret")?
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

fn credential_transport(args: &SecretDeliverArgs) -> Result<skillet_vm::transport::SshTransport> {
    let (user, address) = args
        .target
        .split_once('@')
        .filter(|(user, address)| !user.is_empty() && !address.is_empty())
        .ok_or_else(|| anyhow!("SSH target must be USER@HOST"))?;
    skillet_vm::transport::SshTransport::new(
        skillet_vm::SshTarget {
            user: user.to_string(),
            address: address.to_string(),
            port: args.port,
            identity: args.identity.clone(),
            known_hosts: args.known_hosts.clone(),
        },
        skillet_vm::transport::HostKeyPolicy::Verify,
    )
    .context("validating credential delivery SSH target")
}

#[derive(PartialEq, Eq)]
enum RemoteCredentialState {
    Present,
    Absent,
}

fn remote_credential_state(args: &SecretDeliverArgs) -> Result<RemoteCredentialState> {
    let transport = credential_transport(args)?;
    let program = format!("/var/usrlocal/bin/skillet-{}", args.hostname);
    let output = skillet_vm::transport::GuestTransport::execute(
        &transport,
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &["-n", &program, "credential", "state", "pihole_web_password"],
        },
        None,
    )
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
    Ok(Vault::default_path()?)
}

pub(super) fn provision_vm(args: &VmProvisionArgs) -> Result<()> {
    let profile = skillet_hosts::profile_for_name(&args.hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {}", args.hostname))?;
    if !profile.supports_service("pihole") || !profile.supports_service("tailscale") {
        return Err(anyhow!(
            "host {} must declare both Pi-hole and Tailscale to use VM application provisioning",
            args.hostname
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
    let mut ssh = VmSsh::new(&args.hostname, &identity, &known_hosts, port);
    validate_vm_tailscale_delivery(&mut ssh)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = Vault::open(&vault_path, args.key_file.as_deref())?;
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

pub(super) fn remove_vm_external_resources(args: &VmDestroyArgs) -> Result<()> {
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
    let vault = Vault::open(&vault_path, args.key_file.as_deref())?;
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
    vault: &mut Vault,
    device: &tailscale::DeviceRecord,
) -> Result<()> {
    let environment = UiEnvironmentName::Test.as_str();
    let domain_path = format!("skillet/environments/{environment}/dns/ui-domain");
    let zone_path = format!("skillet/environments/{environment}/dns/cloudflare-zone-id");
    let domain_prefix = vault.get(&domain_path)?;
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let zone_id = zone_id.trim().to_string();
    skillet_workstation::cloudflare::validate_zone_id(&zone_id)?;
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    let zone = api.zone(&creator, &zone_id)?;
    let ui_domain = skillet_caddy::resolve_ui_domain(&zone.name, domain_prefix.as_deref())
        .context("resolving test relative UI domain beneath its Cloudflare zone")?;
    let host_ui = skillet_hosts::ui_config_for_host(&args.hostname)
        .ok_or_else(|| anyhow!("host {} has no declared UI services", args.hostname))?;
    let sites = skillet_caddy::CaddySites::from_host(
        &args.hostname,
        &skillet_caddy::UiEnvironment {
            ui_domain: ui_domain.clone(),
            acme_staging: true,
        },
        &host_ui.services,
    )?;
    let dns = skillet_workstation::cloudflare::desired_records(
        &sites.machine_hostname,
        &device.addresses,
        &sites,
    )?;
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
    let account_id = skillet_workstation::cloudflare::Cloudflare::account_id(&zone)?.to_string();
    write_cloudflare_ownership(&metadata_path, &ownership)?;
    ensure_vault_unchanged(vault)?;
    let issued = api.create_zone_token(
        &creator,
        &ownership.zone_id,
        &account_id,
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
        verify_caddy_denies_non_tailnet_probe(ssh, &sites)?;
        for old_id in api.token_ids_by_name(&creator, &account_id, &ownership.token_name)? {
            if old_id != issued.id {
                api.revoke_token(&creator, &account_id, &old_id)?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })();
    if let Err(error) = work {
        return Err(error.context("provisioning disposable Cloudflare DNS and Caddy"));
    }
    Ok(())
}

fn verify_caddy_denies_non_tailnet_probe(
    ssh: &mut VmSsh<'_>,
    sites: &skillet_caddy::CaddySites,
) -> Result<()> {
    for site in &sites.services {
        for hostname in std::iter::once(&site.hostname).chain(&site.aliases) {
            let probe = format!(
                "curl --insecure --silent --show-error --max-time 8 --resolve '{hostname}:443:127.0.0.1' --write-out '\n%{{http_code}}' 'https://{hostname}/'"
            );
            let mut denied = false;
            for _ in 0..30 {
                let output = ssh.output(&probe)?;
                let response = String::from_utf8_lossy(&output.stdout);
                if output.status.success()
                    && response.trim() == "Access denied by Skillet tailnet policy\n403"
                {
                    denied = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            if !denied {
                return Err(anyhow!("Caddy did not return its explicit access-denied response to a non-tailnet probe"));
            }
        }
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
    skillet_workstation::cloudflare::validate_zone_id(&ownership.zone_id)?;
    skillet_caddy::validate_domain_in_zone(&ownership.ui_domain, &ownership.ui_domain)
        .context("validating Cloudflare UI domain in VM metadata")?;
    let vault_path = args
        .database
        .clone()
        .map_or_else(default_database_path, Ok)?;
    let vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let configured_zone = vault.get("skillet/environments/test/dns/cloudflare-zone-id")?
    .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: skillet/environments/test/dns/cloudflare-zone-id"))?;
    let configured_prefix = vault.get("skillet/environments/test/dns/ui-domain")?;
    if configured_zone.trim() != ownership.zone_id {
        return Err(anyhow!("test Cloudflare configuration differs from the recorded VM owner; restore the original vault values before cleanup"));
    }
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    let zone = api.zone(&creator, &ownership.zone_id)?;
    let account_id = skillet_workstation::cloudflare::Cloudflare::account_id(&zone)?.to_string();
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
        &account_id,
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
        for id in api.token_ids_by_name(&creator, &account_id, &ownership.token_name)? {
            api.revoke_token(&creator, &account_id, &id)?;
        }
        Ok::<(), anyhow::Error>(())
    })();
    let revoke_cleanup = api
        .token_ids_by_name(&creator, &account_id, &cleanup_name)
        .and_then(|ids| {
            for id in ids {
                api.revoke_token(&creator, &account_id, &id)?;
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
    host: &'a str,
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
        let transport = skillet_vm::transport::SshTransport::new(
            skillet_vm::SshTarget {
                user: "giacomo".to_string(),
                address: "127.0.0.1".to_string(),
                port: self.port,
                identity: self.identity.to_path_buf(),
                known_hosts: self.known_hosts.to_path_buf(),
            },
            skillet_vm::transport::HostKeyPolicy::Verify,
        )?;
        skillet_vm::credential::install(
            &transport,
            self.host,
            name,
            unit,
            if defer_start {
                skillet_vm::credential::ActivationPolicy::DeferConsumer
            } else {
                skillet_vm::credential::ActivationPolicy::StartConsumer
            },
            secret.as_bytes(),
        )?;
        Ok(())
    }
}

impl<'a> VmSsh<'a> {
    fn new(host: &'a str, identity: &'a Path, known_hosts: &'a Path, port: u16) -> Self {
        Self {
            host,
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

fn random_password() -> Result<String> {
    let mut bytes = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
#[path = "secret_delivery/tests.rs"]
mod tests;
