use super::{
    butane_root, vm_name, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs, VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use skillet_workstation::provisioning_state;
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
    let mut token_store = skillet_workstation::ui_provisioning::VaultUiTokenStore::new(
        vault,
        args.key_file.as_deref(),
    );
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
    let sites = provisioned.sites;
    let issued = provisioned.token;
    let account_id = provisioned.account_id;
    let token_name = provisioned.token_name;
    let work = (|| {
        let sites_payload = serde_json::to_string(&sites)?;
        skillet_vm::credential::install_set(
            &ssh.transport,
            &args.hostname,
            "skillet-caddy-apply.service",
            skillet_vm::credential::ActivationPolicy::DeferConsumer,
            &[
                ("caddy_sites", sites_payload.as_bytes()),
                ("cloudflare_acme_token", issued.value.as_bytes()),
            ],
        )?;
        ssh.run(
            "/usr/bin/sudo",
            &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
        )?;
        verify_caddy_denies_non_tailnet_probe(ssh, &sites)?;
        for old_id in api.token_ids_by_name(&creator, &account_id, &token_name)? {
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
