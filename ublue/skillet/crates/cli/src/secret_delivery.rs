use super::{
    butane_root, vm_name, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs, VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use skillet_vm::transport::GuestTransport as _;
use skillet_workstation::provisioning_state::{self, CloudflareVmOwnership, ProvisioningIdentity};
use skillet_workstation::tailscale;
use skillet_workstation::vault::Vault;
use std::{
    fs,
    io::Read as _,
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
            let policy = UiEnvironmentName::Production.policy();
            let auth_key = tailscale::create_auth_key(
                &credentials,
                policy.tailscale_tag(
                    skillet_workstation::provisioning_policy::DeviceClass::ProductionHost,
                ),
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
    let policy = args.environment.policy();
    let environment = policy.name();
    let domain_path = policy.ui_domain_entry();
    let domain_prefix = vault.get(&domain_path)?;
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(zone_path)?
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
            acme_staging: policy.acme_staging(),
        },
        &ui_config.services,
    )?;
    let tailnet = tailscale_credentials(vault)?;
    let device = tailscale::find_device_by_hostname(
        &tailnet,
        &args.hostname,
        policy.tailscale_tag(skillet_workstation::provisioning_policy::DeviceClass::ProductionHost),
    )?;
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
    let output = credential_transport(args)?
        .execute(
            &skillet_vm::transport::GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
            },
            None,
        )
        .context("starting Caddy apply after both credentials were delivered")?;
    if !output.status.success() {
        return Err(anyhow!("Caddy apply failed with status {}", output.status));
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
    let environment = args.environment.policy().name();
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
    Ok(tailscale::OAuthCredentials::new(client_id, client_secret)?)
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
    let ssh = VmSsh::new(&args.hostname, &identity, &known_hosts, port)?;
    validate_vm_tailscale_delivery(&ssh)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    let expected_hostname = name.as_str();
    let policy = UiEnvironmentName::Test.policy();
    let provisioning_identity =
        ProvisioningIdentity::new(&args.hostname, policy.name(), &args.instance);

    provisioning_state::mark_tailscale_pending(
        &run_dir,
        &provisioning_identity,
        expected_hostname,
    )?;
    let mut addresses = vm_tailscale_addresses(&ssh).unwrap_or_default();
    if addresses.is_empty() {
        let auth_key = tailscale::create_auth_key(
            &credentials,
            policy
                .tailscale_tag(skillet_workstation::provisioning_policy::DeviceClass::DisposableVm),
            &format!("Skillet disposable VM {expected_hostname}"),
        )?;
        ssh.install("tailscale_auth_key", &auth_key.key)?;
    }

    let pihole_credential = "/etc/credstore.encrypted/skillet/pihole_web_password.cred";
    let has_pihole_credential = vm_credential_present(&ssh, pihole_credential)?;
    if args.rotate || !has_pihole_credential {
        let secret = random_password()?;
        ssh.install("pihole_web_password", &secret)?;
    } else {
        ssh.run(
            "/usr/bin/sudo",
            &["-n", "systemctl", "start", "skillet-full-apply.service"],
        )?;
    }

    addresses = wait_for_vm_tailscale(&ssh)?;
    let record = tailscale::find_device(
        &credentials,
        expected_hostname,
        policy.tailscale_tag(skillet_workstation::provisioning_policy::DeviceClass::DisposableVm),
        &addresses,
    )?;
    provisioning_state::save_tailscale_record(&run_dir, &provisioning_identity, &record)?;
    if args.with_ui {
        provision_vm_ui(args, &run_dir, &ssh, &mut vault, &record)?;
    }
    provisioning_state::remove_tailscale_pending(&run_dir)?;
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
    let policy = UiEnvironmentName::Test.policy();
    let provisioning_identity =
        ProvisioningIdentity::new(&args.hostname, policy.name(), &args.instance);
    let expected = if record_path.exists() {
        Some(provisioning_state::load_tailscale_record(
            &record_path,
            &provisioning_identity,
            &name,
        )?)
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
        policy.tailscale_tag(skillet_workstation::provisioning_policy::DeviceClass::DisposableVm),
        expected.as_ref(),
    )?
    .is_some()
    {
        println!("Removed Tailscale device for {name}");
    }
    provisioning_state::remove_tailscale_pending(&run_dir)?;
    if record_path.exists() {
        fs::remove_file(record_path).context("removing Tailscale VM metadata")?;
    }
    Ok(())
}

fn provision_vm_ui(
    args: &VmProvisionArgs,
    run_dir: &Path,
    ssh: &VmSsh,
    vault: &mut Vault,
    device: &tailscale::DeviceRecord,
) -> Result<()> {
    let policy = UiEnvironmentName::Test.policy();
    let environment = policy.name();
    let domain_path = policy.ui_domain_entry();
    let zone_path = policy.cloudflare_zone_entry();
    let domain_prefix = vault.get(&domain_path)?;
    let zone_id = vault
        .get(zone_path)?
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
    let sites = test_vm_caddy_sites(&args.hostname, &ui_domain, policy.acme_staging())?;
    let dns = skillet_workstation::cloudflare::desired_records(
        &sites.machine_hostname,
        &device.addresses,
        &sites,
    )?;
    let marker = format!("skillet:test:{}:{}", args.hostname, args.instance);
    let token_name = format!("skillet:test:{}-{}", args.hostname, args.instance);
    let metadata_path = run_dir.join("cloudflare.json");
    let mut ownership = CloudflareVmOwnership {
        identity: Some(ProvisioningIdentity::new(
            &args.hostname,
            environment,
            &args.instance,
        )),
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
    provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
    ensure_vault_unchanged(vault)?;
    let issued = api.create_zone_token(
        &creator,
        &ownership.zone_id,
        &account_id,
        &ownership.token_name,
        policy.cloudflare_token_lifetime(),
    )?;
    ownership.token_id = Some(issued.id.clone());
    ownership.expires_on.clone_from(&issued.expires_on);
    provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
    let work = (|| {
        let owned = api.reconcile_dns(
            &issued.value,
            &ownership.zone_id,
            &ownership.marker,
            &ownership.ui_domain,
            &dns,
        )?;
        ownership.record_ids = owned.records.into_iter().map(|record| record.id).collect();
        provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
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
        ssh.run(
            "/usr/bin/sudo",
            &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
        )?;
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

fn test_vm_caddy_sites(
    hostname: &str,
    ui_domain: &str,
    acme_staging: bool,
) -> Result<skillet_caddy::CaddySites> {
    let host_ui = skillet_hosts::ui_config_for_host(hostname)
        .ok_or_else(|| anyhow!("host {hostname} has no declared UI services"))?;
    Ok(skillet_caddy::CaddySites::from_host(
        hostname,
        &skillet_caddy::UiEnvironment {
            ui_domain: ui_domain.to_string(),
            acme_staging,
        },
        &host_ui.services,
    )?)
}

fn verify_caddy_denies_non_tailnet_probe(
    ssh: &VmSsh,
    sites: &skillet_caddy::CaddySites,
) -> Result<()> {
    for site in &sites.services {
        for hostname in std::iter::once(&site.hostname).chain(&site.aliases) {
            let resolve = format!("{hostname}:443:127.0.0.1");
            let url = format!("https://{hostname}/");
            let mut denied = false;
            for _ in 0..30 {
                let output = ssh.output(
                    "/usr/bin/curl",
                    &[
                        "--insecure",
                        "--silent",
                        "--show-error",
                        "--max-time",
                        "8",
                        "--resolve",
                        &resolve,
                        "--write-out",
                        "\n%{http_code}",
                        &url,
                    ],
                )?;
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
    let ownership = provisioning_state::load_cloudflare_ownership(metadata_path)
        .context("reading Cloudflare VM ownership metadata")?;
    let policy = UiEnvironmentName::Test.policy();
    let expected_identity =
        ProvisioningIdentity::new(&args.hostname, policy.name(), &args.instance);
    provisioning_state::validate_cloudflare_identity(&ownership, &expected_identity)
        .context("Cloudflare VM cleanup identity mismatch")?;
    if ownership.environment != policy.name() {
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
    let configured_zone = vault.get(policy.cloudflare_zone_entry())?.ok_or_else(|| {
        anyhow!(
            "KeePassXC Cloudflare zone entry is missing: {}",
            policy.cloudflare_zone_entry()
        )
    })?;
    let configured_prefix = vault.get(&policy.ui_domain_entry())?;
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
        Some(policy.cleanup_token_lifetime()),
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

struct VmSsh {
    host: String,
    transport: skillet_vm::transport::SshTransport,
}

impl VmSsh {
    fn new(host: &str, identity: &Path, known_hosts: &Path, port: u16) -> Result<Self> {
        Ok(Self {
            host: host.to_string(),
            transport: skillet_vm::transport::SshTransport::new(
                skillet_vm::SshTarget {
                    user: "giacomo".to_string(),
                    address: "127.0.0.1".to_string(),
                    port,
                    identity: identity.to_path_buf(),
                    known_hosts: known_hosts.to_path_buf(),
                },
                skillet_vm::transport::HostKeyPolicy::Verify,
            )?,
        })
    }

    fn output(&self, program: &str, arguments: &[&str]) -> Result<std::process::Output> {
        use skillet_vm::transport::GuestTransport as _;
        self.transport
            .execute(
                &skillet_vm::transport::GuestCommand { program, arguments },
                None,
            )
            .context("running command over recorded VM SSH transport")
    }

    fn run(&self, program: &str, arguments: &[&str]) -> Result<()> {
        let output = self.output(program, arguments)?;
        if !output.status.success() {
            return Err(anyhow!(
                "VM command {program} failed with status {}",
                output.status
            ));
        }
        Ok(())
    }

    fn install(&self, name: &str, secret: &str) -> Result<()> {
        self.install_for_unit(
            name,
            "skillet-full-apply.service",
            secret,
            skillet_vm::credential::ActivationPolicy::StartConsumer,
        )
    }

    fn install_deferred(&self, name: &str, unit: &str, secret: &str) -> Result<()> {
        self.install_for_unit(
            name,
            unit,
            secret,
            skillet_vm::credential::ActivationPolicy::DeferConsumer,
        )
    }

    fn install_for_unit(
        &self,
        name: &str,
        unit: &str,
        secret: &str,
        activation: skillet_vm::credential::ActivationPolicy,
    ) -> Result<()> {
        skillet_vm::credential::install(
            &self.transport,
            &self.host,
            name,
            unit,
            activation,
            secret.as_bytes(),
        )?;
        Ok(())
    }
}

fn vm_credential_present(ssh: &VmSsh, path: &str) -> Result<bool> {
    let output = ssh.output("/usr/bin/sudo", &["-n", "test", "-s", path])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(anyhow!("could not inspect VM credential state")),
    }
}

fn vm_tailscale_addresses(ssh: &VmSsh) -> Result<std::collections::BTreeSet<String>> {
    let output = ssh.output(
        "/usr/bin/sudo",
        &[
            "-n",
            "podman",
            "exec",
            "tailscale",
            "tailscale",
            "status",
            "--json",
        ],
    )?;
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

fn wait_for_vm_tailscale(ssh: &VmSsh) -> Result<std::collections::BTreeSet<String>> {
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

fn validate_vm_tailscale_delivery(ssh: &VmSsh) -> Result<()> {
    let output = ssh
        .output(
            "/usr/bin/sudo",
            &["-n", "systemctl", "cat", "skillet-full-apply.service"],
        )
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
