use super::{
    butane_root, vm_name, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs, VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use skillet_workstation::provisioning_state;
use skillet_workstation::tailscale;
use skillet_workstation::vault::Vault;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    validate_delivery_service(&args.hostname, &args.service)?;
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

fn deliver_caddy_from_vault(args: &SecretDeliverArgs, vault: &mut Vault) -> Result<()> {
    let policy = args.environment.policy();
    let domain_path = policy.ui_domain_entry();
    let domain_prefix = vault.get(&domain_path)?;
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(zone_path)?
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
    let ssh = VmSsh::new(&identity, &known_hosts, port)?;
    validate_vm_tailscale_delivery(&ssh)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    let expected_hostname = name.as_str();
    let policy = UiEnvironmentName::Test.policy();
    let record = skillet_workstation::tailscale_enrollment::enroll_disposable_vm(
        &skillet_workstation::tailscale_enrollment::DisposableEnrollment {
            host: &args.hostname,
            instance: &args.instance,
            vm_hostname: expected_hostname,
            run_directory: &run_dir,
            policy,
        },
        &credentials,
        &ssh.transport,
    )?;

    skillet_workstation::credential_delivery::ensure_disposable_pihole_credential(
        &args.hostname,
        args.rotate,
        &ssh.transport,
    )?;

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
    let cloudflare_path = run_dir.join("cloudflare.json");
    let cloudflare_exists = provisioning_state::cloudflare_ownership_exists(&cloudflare_path)?;
    let tailscale_pending = provisioning_state::tailscale_pending_exists(&run_dir)?;
    let tailscale_record = provisioning_state::tailscale_record_exists(&run_dir)?;
    if !tailscale_pending && !tailscale_record && !cloudflare_exists {
        return Ok(());
    }
    if cloudflare_exists {
        cleanup_vm_cloudflare(args, &cloudflare_path)?;
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
            vm_hostname: &name,
            run_directory: &run_dir,
            policy,
        },
        &credentials,
    )?;
    println!("Cleaned Tailscale ownership for {name}");
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
        &ssh.transport,
        std::time::Duration::from_mins(1),
        std::time::Duration::from_secs(2),
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
    let configured_zone = vault.get(policy.cloudflare_zone_entry())?.ok_or_else(|| {
        anyhow!(
            "KeePassXC Cloudflare zone entry is missing: {}",
            policy.cloudflare_zone_entry()
        )
    })?;
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

struct VmSsh {
    transport: skillet_vm::transport::SshTransport,
}

impl VmSsh {
    fn new(identity: &Path, known_hosts: &Path, port: u16) -> Result<Self> {
        Ok(Self {
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

#[cfg(test)]
#[path = "secret_delivery/tests.rs"]
mod tests;
