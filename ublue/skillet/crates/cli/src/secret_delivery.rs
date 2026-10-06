use super::{butane_root, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs, VmProvisionArgs};
use anyhow::{anyhow, Context, Result};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    Environment, ManifestStore, Phase, RunIdentity, VmRun,
};
use skillet_workstation::provisioning_state;
use skillet_workstation::tailscale;
use skillet_workstation::vault::Vault;
use std::path::Path;

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    validate_delivery_service(&args.hostname, &args.service)?;
    if args.service == "ddns"
        && args.environment.policy().environment()
            != skillet_workstation::provisioning_policy::Environment::Production
    {
        return Err(anyhow!(
            "disposable DDNS is not enabled until record ownership and cleanup are implemented"
        ));
    }
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
            let transport = credential_transport(args)?;
            let mut store = skillet_workstation::vault::VaultSecretStore::new(
                &mut vault,
                args.key_file.as_deref(),
            );
            skillet_workstation::credential_delivery::deliver_pihole_credential(
                &args.hostname,
                &mut store,
                &transport,
            )?;
            Ok(())
        }
        "tailscale" => {
            let credentials = tailscale_credentials(&vault)?;
            let policy = UiEnvironmentName::Production.policy();
            let transport = credential_transport(args)?;
            skillet_workstation::credential_delivery::deliver_tailscale_credential(
                &args.hostname,
                policy,
                &credentials,
                &transport,
            )?;
            Ok(())
        }
        "caddy" => deliver_caddy_from_vault(args, &mut vault),
        "ddns" => deliver_ddns_from_vault(args, &mut vault),
        _ => Err(anyhow!("unsupported secret service {}", args.service)),
    }
}

fn validate_delivery_service(hostname: &str, service: &str) -> Result<skillet_hosts::HostProfile> {
    let profile = skillet_hosts::profile_for_name(hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {hostname}"))?;
    match service {
        "pihole" | "tailscale" | "ddns" if !profile.supports_service(service) => Err(anyhow!(
            "host {hostname} does not declare {service} credential delivery"
        )),
        "caddy" if profile.ui_services().is_empty() => {
            Err(anyhow!("host {hostname} declares no UI services"))
        }
        "pihole" | "tailscale" | "caddy" | "ddns" => Ok(profile),
        unsupported => Err(anyhow!("unsupported secret service {unsupported}")),
    }
}

fn deliver_ddns_from_vault(args: &SecretDeliverArgs, vault: &mut Vault) -> Result<()> {
    let policy = args.environment.policy();
    if policy.environment() != skillet_workstation::provisioning_policy::Environment::Production {
        return Err(anyhow!(
            "disposable DDNS is not enabled until record ownership and cleanup are implemented"
        ));
    }
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let config_path = format!(
        "skillet/environments/{}/hosts/{}/cloudflare/ddns-config",
        policy.vault_name(),
        args.hostname
    );
    let config = vault
        .get(&config_path)?
        .ok_or_else(|| anyhow!("KeePassXC DDNS config is missing: {config_path}"))?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare token creator is missing"))?;
    let prefix = vault.get(&policy.ui_domain_entry())?;
    let transport = credential_transport(args)?;
    let provider = skillet_workstation::cloudflare::Cloudflare::new();
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::ddns_provisioning::deliver_persistent_ddns(
        &skillet_workstation::ddns_provisioning::DdnsDelivery {
            host: &args.hostname,
            policy,
            zone_id: zone_id.trim(),
            relative_ui_domain: prefix.as_deref(),
            config: &config,
            creator_token: &creator,
        },
        &mut store,
        &provider,
        &transport,
    )?;
    Ok(())
}

fn deliver_caddy_from_vault(args: &SecretDeliverArgs, vault: &mut Vault) -> Result<()> {
    let policy = args.environment.policy();
    let domain_path = policy.ui_domain_entry();
    let domain_prefix = vault.get(&domain_path)?;
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let zone_id = zone_id.trim().to_string();
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| {
            anyhow!(
                "KeePassXC Cloudflare token creator is missing: skillet/cloudflare/token-creator"
            )
        })?;
    let tailnet = tailscale_credentials(vault)?;
    let transport = credential_transport(args)?;
    let cloudflare = skillet_workstation::cloudflare::Cloudflare::new();
    let mut token_store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::ui_provisioning::deliver_persistent_ui(
        &skillet_workstation::ui_provisioning::PersistentUiDelivery {
            host: &args.hostname,
            policy,
            zone_id: &zone_id,
            relative_ui_domain: domain_prefix.as_deref(),
            creator_token: &creator,
        },
        &mut token_store,
        &cloudflare,
        &tailnet,
        &transport,
    )?;
    Ok(())
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

pub(super) fn unlock_vault(path: Option<&Path>, key_file: Option<&Path>) -> Result<()> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_database_path()?,
    };
    let vault = Vault::open(&path, key_file).context("opening KeePassXC database for unlock")?;
    if !vault.password_cached() {
        return Err(anyhow!(
            "KeePassXC password was verified, but the three-hour session cache could not be established; see the keyring warning above"
        ));
    }
    drop(vault);
    println!("KeePassXC vault unlocked; password cached for up to three hours");
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
    if args.with_ddns && !profile.supports_service("ddns") {
        return Err(anyhow!("host {} does not declare DDNS", args.hostname));
    }
    let butane = butane_root()?;
    let run_identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&run_identity)?;
    let run = store.load(&run_identity)?;
    if run.environment != Environment::Test || !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(anyhow!("VM must be a started test run before provisioning"));
    }
    let run_dir = store.run_dir(&run_identity);
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let base_transport = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let verify_ownership = || verify_running_vm(&backend, &run);
    verify_ownership()?;
    let transport = OwnershipCheckedTransport::new(&base_transport, &verify_ownership);
    validate_vm_tailscale_delivery(&transport)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    let expected_hostname = run.guest_hostname.as_str();
    let policy = UiEnvironmentName::Test.policy();
    // The host full-apply unit requires both encrypted credentials. Deliver
    // Pi-hole first; Tailscale delivery then starts full apply with the full
    // credential set, which creates the Tailscale container on a fresh VM.
    skillet_workstation::credential_delivery::ensure_disposable_pihole_credential(
        &args.hostname,
        args.rotate,
        &transport,
    )?;
    let record = skillet_workstation::tailscale_enrollment::enroll_disposable_vm(
        &skillet_workstation::tailscale_enrollment::DisposableEnrollment {
            host: &args.hostname,
            instance: &args.instance,
            vm_hostname: expected_hostname,
            run_directory: &run_dir,
            policy,
        },
        &credentials,
        &transport,
    )?;

    if args.with_ui {
        provision_vm_ui(args, &run_dir, &transport, &mut vault, &record)?;
    }
    if args.with_ddns {
        provision_vm_ddns(args, &run_dir, &transport, &mut vault)?;
    }
    provisioning_state::remove_tailscale_pending(&run_dir)?;
    println!("Tailscale connected VM {expected_hostname}");
    Ok(())
}

pub(super) fn remove_vm_external_resources(args: &VmDestroyArgs, run: &VmRun) -> Result<()> {
    if run.identity.host() != args.hostname || run.identity.instance() != args.instance {
        return Err(anyhow!(
            "destroy arguments do not match the recorded VM identity"
        ));
    }
    let butane = butane_root()?;
    let run_dir = butane.join("runs").join(run.identity.domain_name());
    let cloudflare_path = run_dir.join("cloudflare.json");
    let ddns_path = run_dir.join("ddns.json");
    let cloudflare_exists = provisioning_state::cloudflare_ownership_exists(&cloudflare_path)?;
    let ddns_exists = provisioning_state::ddns_ownership_exists(&ddns_path)?;
    let tailscale_pending = provisioning_state::tailscale_pending_exists(&run_dir)?;
    let tailscale_record = provisioning_state::tailscale_record_exists(&run_dir)?;
    if !tailscale_pending && !tailscale_record && !cloudflare_exists && !ddns_exists {
        return Ok(());
    }
    if cloudflare_exists {
        cleanup_vm_cloudflare(args, &cloudflare_path)?;
    }
    if ddns_exists {
        cleanup_vm_ddns(args, &ddns_path)?;
    }
    if !provisioning_state::tailscale_pending_exists(&run_dir)?
        && !provisioning_state::tailscale_record_exists(&run_dir)?
    {
        return Ok(());
    }
    let policy = UiEnvironmentName::Test.policy();
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    skillet_workstation::tailscale_enrollment::cleanup_disposable_vm(
        &skillet_workstation::tailscale_enrollment::DisposableCleanup {
            host: &args.hostname,
            instance: &args.instance,
            vm_hostname: &run.identity.domain_name(),
            run_directory: &run_dir,
            policy,
        },
        &credentials,
    )?;
    println!(
        "Cleaned Tailscale ownership for {}",
        run.identity.domain_name()
    );
    Ok(())
}

fn provision_vm_ui(
    args: &VmProvisionArgs,
    run_dir: &Path,
    guest: &impl GuestTransport,
    vault: &mut Vault,
    device: &tailscale::DeviceRecord,
) -> Result<()> {
    let policy = UiEnvironmentName::Test.policy();
    let domain_path = policy.ui_domain_entry();
    let zone_path = policy.cloudflare_zone_entry();
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
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    let provisioned = skillet_workstation::ui_provisioning::provision_disposable_ui(
        &skillet_workstation::ui_provisioning::DisposableUiRequest {
            host: &args.hostname,
            instance: &args.instance,
            policy,
            zone_id: &zone_id,
            relative_ui_domain: domain_prefix.as_deref(),
            creator_token: &creator,
            run_directory: run_dir,
            addresses: &device.addresses,
        },
        &api,
        || vault.ensure_unchanged(),
    )?;
    skillet_workstation::ui_provisioning::deliver_disposable_ui(
        &args.hostname,
        &provisioned,
        &creator,
        &api,
        guest,
        std::time::Duration::from_mins(1),
        std::time::Duration::from_secs(2),
    )?;
    Ok(())
}

fn provision_vm_ddns(
    args: &VmProvisionArgs,
    run_dir: &Path,
    guest: &impl GuestTransport,
    vault: &mut Vault,
) -> Result<()> {
    let policy = UiEnvironmentName::Test.policy();
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let config_path = format!(
        "skillet/environments/{}/hosts/{}/cloudflare/ddns-config",
        policy.vault_name(),
        args.hostname
    );
    let config = vault
        .get(&config_path)?
        .ok_or_else(|| anyhow!("KeePassXC DDNS config is missing: {config_path}"))?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare token creator is missing"))?;
    let prefix = vault.get(&policy.ui_domain_entry())?;
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    skillet_workstation::ddns_provisioning::provision_disposable_ddns(
        &skillet_workstation::ddns_provisioning::DisposableDdnsRequest {
            host: &args.hostname,
            instance: &args.instance,
            policy,
            zone_id: zone_id.trim(),
            relative_ui_domain: prefix.as_deref(),
            config: &config,
            creator_token: &creator,
            run_directory: run_dir,
        },
        &api,
        guest,
        std::time::Duration::from_mins(5),
        std::time::Duration::from_secs(5),
    )?;
    Ok(())
}

fn cleanup_vm_ddns(args: &VmDestroyArgs, metadata_path: &Path) -> Result<()> {
    let policy = UiEnvironmentName::Test.policy();
    let vault_path = args
        .database
        .clone()
        .map_or_else(default_database_path, Ok)?;
    let vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare token creator is missing"))?;
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    skillet_workstation::ddns_provisioning::cleanup_disposable_ddns(
        &skillet_workstation::ddns_provisioning::DisposableDdnsCleanup {
            host: &args.hostname,
            instance: &args.instance,
            policy,
            ownership_path: metadata_path,
            configured_zone_id: zone_id.trim(),
            creator_token: &creator,
        },
        &api,
    )?;
    Ok(())
}

fn cleanup_vm_cloudflare(args: &VmDestroyArgs, metadata_path: &Path) -> Result<()> {
    let policy = UiEnvironmentName::Test.policy();
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
    let zone_path = policy.cloudflare_zone_entry();
    let configured_zone = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let api = skillet_workstation::cloudflare::Cloudflare::new();
    let configured_prefix = vault.get(&policy.ui_domain_entry())?;
    skillet_workstation::ui_provisioning::cleanup_disposable_ui(
        &skillet_workstation::ui_provisioning::DisposableUiCleanupRequest {
            host: &args.hostname,
            instance: &args.instance,
            policy,
            ownership_path: metadata_path.to_path_buf(),
            configured_zone_id: &configured_zone,
            relative_ui_domain: configured_prefix.as_deref(),
            creator_token: &creator,
        },
        &api,
    )?;
    Ok(())
}

fn verify_running_vm(backend: &impl VmBackend, run: &VmRun) -> skillet_vm::Result<()> {
    let domain = backend
        .inspect(run)?
        .ok_or_else(|| skillet_vm::Error::Invalid("owned VM domain is absent".into()))?;
    domain.validate_owned(run)?;
    if domain.state != "running" {
        return Err(skillet_vm::Error::Invalid(format!(
            "VM is not running (libvirt state: {})",
            domain.state
        )));
    }
    Ok(())
}

fn validate_vm_tailscale_delivery(guest: &impl GuestTransport) -> Result<()> {
    let output = guest
        .execute(
            &GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &["-n", "systemctl", "cat", "skillet-full-apply.service"],
            },
            None,
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

#[cfg(test)]
#[path = "secret_delivery/tests.rs"]
mod tests;
