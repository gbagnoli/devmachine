use super::{
    butane_root, SecretCheckArgs, SecretDeliverArgs, UiEnvironmentName, VmDestroyArgs,
    VmProvisionArgs,
};
use anyhow::{anyhow, Context, Result};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    Environment, ManifestStore, Phase, RunIdentity, VmRun,
};
use skillet_workstation::provisioning_policy::ProvisioningPolicy;
use skillet_workstation::provisioning_state;
use skillet_workstation::tailscale;
use skillet_workstation::vault::Vault;
use std::path::Path;

pub(super) fn deliver_from_vault(args: &SecretDeliverArgs) -> Result<()> {
    let profile = validate_delivery_service(&args.hostname, &args.service)?;
    if args.environment.policy().environment()
        != skillet_workstation::provisioning_policy::Environment::Production
    {
        let service = if args.service == "ddns" {
            "DDNS"
        } else if args.service == "smtp" {
            "SMTP"
        } else if args.service == "datadog" {
            "Datadog"
        } else {
            args.service.as_str()
        };
        return Err(anyhow!(
            "test {service} must be provisioned through an owned disposable VM"
        ));
    }
    if args.service == "pihole"
        && !profile
            .credential_consumers()
            .iter()
            .any(|consumer| consumer.credential == skillet_hosts::PIHOLE_WEB_PASSWORD_CREDENTIAL)
    {
        println!("Pi-hole uses tailnet identity authentication; no password delivery is needed. Use `skillet apply {}` to converge the service and Caddy policy", args.hostname);
        return Ok(());
    }
    let database = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let transport = credential_transport(args)?;
    let mut vault = Vault::open(&database, args.key_file.as_deref())?;
    let activation = if args.activate {
        skillet_vm::credential::ActivationPolicy::StartConsumer
    } else {
        skillet_vm::credential::ActivationPolicy::DeferConsumer
    };
    if !deliver_with_vault(args, &mut vault, &transport, activation)? {
        return Ok(());
    }
    if args.activate && args.service == "network" {
        println!("Prepared native network profiles for the next boot; live connections were not switched");
    } else if args.activate {
        println!(
            "Delivered {} credentials and requested consumer activation",
            args.service
        );
    } else {
        println!("Staged {} credentials; use remote apply to configure all declared services, or --activate for this consumer", args.service);
    }
    Ok(())
}

pub(super) fn deliver_with_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<bool> {
    validate_delivery_service(&args.hostname, &args.service)?;
    match args.service.as_str() {
        "pihole" => {
            let mut store =
                skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
            skillet_workstation::credential_delivery::deliver_pihole_credential(
                &args.hostname,
                &mut store,
                transport,
                activation,
            )?;
        }
        "tailscale" => {
            let credentials = tailscale_credentials(vault)?;
            skillet_workstation::credential_delivery::deliver_tailscale_credential(
                &args.hostname,
                args.environment.policy(),
                &credentials,
                transport,
                activation,
            )?;
        }
        "pihole-dns" => skillet_workstation::pihole_dns::deliver(
            &args.hostname,
            args.environment.policy(),
            vault,
            transport,
            activation,
        )?,
        "syncthing" => skillet_workstation::syncthing_provisioning::deliver(
            &args.hostname,
            &args.hostname,
            args.environment.policy(),
            vault,
            transport,
            activation,
        )?,
        "homeassistant" => {
            let mut store =
                skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
            if !skillet_workstation::homeassistant_provisioning::deliver(
                &args.hostname,
                args.environment.policy(),
                &mut store,
                transport,
                activation,
            )? {
                println!("Home Assistant backup delivery skipped: add the post-onboarding API token to KeePassXC to enable encrypted scheduled backups");
                return Ok(false);
            }
        }
        "joplin" => deliver_joplin_from_vault(args, vault, transport, activation)?,
        "airtrail" => deliver_airtrail_from_vault(args, vault, transport, activation)?,
        "media" => deliver_media_from_vault(args, vault, transport, activation)?,
        "caddy" => deliver_caddy_from_vault(args, vault, transport, activation)?,
        "ddns" => deliver_ddns_from_vault(args, vault, transport, activation)?,
        "network" => {
            let policy = args.environment.policy();
            let input = skillet_workstation::network_provisioning::input(
                &args.hostname,
                policy,
                &|path, field| vault.get_field(path, field),
            )?;
            skillet_workstation::network_provisioning::deliver(
                &args.hostname,
                policy,
                &input,
                transport,
                activation,
            )?;
        }
        "smtp" => {
            let input = skillet_workstation::smtp_provisioning::input(
                args.environment.policy(),
                &|path, field| vault.get_field(path, field),
            )?;
            skillet_workstation::smtp_provisioning::deliver(
                &args.hostname,
                &input,
                transport,
                activation,
            )?;
        }
        "datadog" => deliver_datadog_from_vault(
            &args.hostname,
            args.environment.policy(),
            vault,
            transport,
            activation,
        )?,
        _ => return Err(anyhow!("unsupported secret service {}", args.service)),
    }
    Ok(true)
}

fn deliver_media_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::media_provisioning::deliver(
        &args.hostname,
        None,
        args.environment.policy(),
        &mut store,
        transport,
        activation,
    )?;
    Ok(())
}

fn deliver_airtrail_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::airtrail_provisioning::deliver_production(
        &args.hostname,
        &mut store,
        transport,
        activation,
    )?;
    Ok(())
}

fn deliver_joplin_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::joplin_provisioning::deliver_production(
        &args.hostname,
        &mut store,
        transport,
        activation,
    )?;
    Ok(())
}

fn validate_delivery_service(hostname: &str, service: &str) -> Result<skillet_hosts::HostProfile> {
    let profile = skillet_hosts::profile_for_name(hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {hostname}"))?;
    if service == "pihole-dns" && profile.supports_service("pihole") {
        return Ok(profile);
    }
    if service == "ddns" && !profile.supports_production_service("ddns") {
        return Err(anyhow!(
            "host {hostname} has no production DDNS credential delivery"
        ));
    }
    match service {
        "pihole" | "tailscale" | "ddns" | "datadog" | "smtp" | "network" | "syncthing"
        | "homeassistant" | "joplin" | "airtrail" | "media"
            if !profile.supports_service(service) =>
        {
            Err(anyhow!(
                "host {hostname} does not declare {service} credential delivery"
            ))
        }
        "caddy" if profile.ui_services().is_empty() => {
            Err(anyhow!("host {hostname} declares no UI services"))
        }
        "pihole" | "tailscale" | "caddy" | "ddns" | "datadog" | "smtp" | "network"
        | "syncthing" | "homeassistant" | "joplin" | "airtrail" | "media" => Ok(profile),
        unsupported => Err(anyhow!("unsupported secret service {unsupported}")),
    }
}

fn deliver_datadog_from_vault(
    host: &str,
    policy: skillet_workstation::provisioning_policy::ProvisioningPolicy,
    vault: &Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let input = skillet_workstation::configuration_templates::render(
        "datadog",
        host,
        policy.vault_name(),
        vault,
    )?;
    skillet_workstation::credential_delivery::deliver_datadog_credential(
        host, policy, &input, transport, activation,
    )?;
    Ok(())
}

fn deliver_ddns_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let policy = args.environment.policy();
    if policy.environment() != skillet_workstation::provisioning_policy::Environment::Production {
        return Err(anyhow!(
            "test DDNS must be provisioned through an owned disposable VM"
        ));
    }
    let zone_path = policy.cloudflare_zone_entry();
    let zone_id = vault
        .get(&zone_path)?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare zone entry is missing: {zone_path}"))?;
    let config = skillet_workstation::configuration_templates::render(
        "ddns",
        &args.hostname,
        policy.vault_name(),
        vault,
    )?;
    let creator = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or_else(|| anyhow!("KeePassXC Cloudflare token creator is missing"))?;
    let prefix = vault.get(&policy.ui_domain_entry())?;
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
        transport,
        activation,
    )?;
    Ok(())
}

fn deliver_caddy_from_vault(
    args: &SecretDeliverArgs,
    vault: &mut Vault,
    transport: &impl GuestTransport,
    activation: skillet_vm::credential::ActivationPolicy,
) -> Result<()> {
    let users = tailnet_ui_users(&args.hostname, vault)?;
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
    let homeassistant_api_token = if skillet_hosts::profile_for_name(&args.hostname)
        .is_some_and(|profile| profile.supports_service("homeassistant"))
    {
        vault.get(&format!(
            "skillet/environments/{}/hosts/{}/homeassistant/api-token",
            policy.vault_name(),
            args.hostname
        ))?
    } else {
        None
    };
    let cloudflare = skillet_workstation::cloudflare::Cloudflare::new();
    let mut token_store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::ui_provisioning::deliver_persistent_ui(
        &skillet_workstation::ui_provisioning::PersistentUiDelivery {
            public_targets: &args.public_target,
            host: &args.hostname,
            policy,
            zone_id: &zone_id,
            relative_ui_domain: domain_prefix.as_deref(),
            tailnet_users: &users,
            creator_token: &creator,
            homeassistant_api_token: homeassistant_api_token.as_deref(),
        },
        &mut token_store,
        &cloudflare,
        &tailnet,
        transport,
        activation,
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

pub(super) fn check_vault(args: &SecretCheckArgs) -> Result<()> {
    let all_profiles = skillet_hosts::profile::declared_profiles();
    let profiles = if let Some(host) = &args.host {
        vec![skillet_hosts::profile_for_name(host)
            .ok_or_else(|| anyhow!("unknown host profile: {host}"))?]
    } else {
        all_profiles.clone()
    };
    let path = args
        .database
        .clone()
        .map_or_else(default_database_path, Ok)?;
    let vault = Vault::open(&path, args.key_file.as_deref())?;
    let report = skillet_workstation::secrets::check_with_entries(
        &profiles,
        args.environment.policy(),
        &vault.entry_paths(),
        &|path, field| vault.get_field(path, field),
    )?;
    let unused = skillet_workstation::secrets::unused_paths(&all_profiles, &vault.entry_paths())?;
    let hyperlinks = std::io::IsTerminal::is_terminal(&std::io::stdout())
        && supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout);
    print!(
        "{}",
        crate::secret_output::render(&report, &unused, hyperlinks)
    );
    if !report.missing.is_empty() {
        return Err(anyhow!(
            "required vault entries need attention; see instructions above"
        ));
    }
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

pub(super) fn credential_transport(args: &SecretDeliverArgs) -> Result<SshTransport> {
    super::ssh::transport(
        &args.target,
        args.port,
        args.identity.as_deref(),
        args.known_hosts.as_deref(),
    )
}

fn default_database_path() -> Result<std::path::PathBuf> {
    Ok(Vault::default_path()?)
}

fn validate_vm_provision(
    args: &VmProvisionArgs,
    profile: &skillet_hosts::HostProfile,
) -> Result<()> {
    if args.with_ui {
        skillet_workstation::ui_provisioning::validate_public_targets(
            &args.hostname,
            &args.public_target,
        )?;
    }
    if args.smtp_only && !profile.supports_service("smtp") {
        return Err(anyhow!("host {} does not declare SMTP", args.hostname));
    }
    if !args.smtp_only
        && (!profile.supports_service("pihole") || !profile.supports_service("tailscale"))
    {
        return Err(anyhow!(
            "host {} must declare both Pi-hole and Tailscale to use VM application provisioning",
            args.hostname
        ));
    }
    if args.with_ddns && !profile.supports_service("ddns") {
        return Err(anyhow!("host {} does not declare DDNS", args.hostname));
    }
    if args.with_datadog && !profile.supports_service("datadog") {
        return Err(anyhow!("host {} does not declare Datadog", args.hostname));
    }
    if args.smtp_only
        && (args.with_ui
            || args.with_ddns
            || args.with_datadog
            || args.with_pihole_dns
            || args.rotate)
    {
        return Err(anyhow!(
            "--smtp-only cannot be combined with application provisioning switches"
        ));
    }
    Ok(())
}

pub(super) fn provision_vm(args: &VmProvisionArgs) -> Result<()> {
    let profile = skillet_hosts::profile_for_name(&args.hostname)
        .ok_or_else(|| anyhow!("unknown host profile: {}", args.hostname))?;
    validate_vm_provision(args, &profile)?;
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
    if args.smtp_only {
        return provision_vm_smtp(args, &transport);
    }
    if args.with_pihole_dns {
        skillet_workstation::host_apply::activate(&transport, "skillet-apply.service")?;
    }
    validate_vm_tailscale_delivery(&transport)?;
    let vault_path = match &args.database {
        Some(path) => path.clone(),
        None => default_database_path()?,
    };
    let mut vault = Vault::open(&vault_path, args.key_file.as_deref())?;
    let credentials = tailscale_credentials(&vault)?;
    let expected_hostname = run.guest_hostname.as_str();
    let policy = UiEnvironmentName::Test.policy();
    // Native-password callers stage their optional Pi-hole credential first.
    // Identity-protected callers need none; Tailscale delivery starts full apply
    // only after the Syncthing bootstrap and required credentials are ready.
    skillet_workstation::credential_delivery::ensure_disposable_pihole_credential(
        &args.hostname,
        args.rotate,
        &transport,
    )?;
    provision_vm_syncthing(args, expected_hostname, &vault, &transport)?;
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

    // Enrollment reuse does not converge changed service/network declarations.
    // Both encrypted prerequisites and the optional bootstrap are ready here.
    skillet_workstation::host_apply::activate(&transport, "skillet-full-apply.service")?;
    if args.with_pihole_dns {
        skillet_workstation::pihole_dns::deliver(
            &args.hostname,
            policy,
            &vault,
            &transport,
            skillet_vm::credential::ActivationPolicy::StartConsumer,
        )?;
    }
    if args.with_ui {
        let sites = provision_vm_ui(args, &run_dir, &transport, &mut vault, &record)?;
        provision_vm_joplin(args, &profile, policy, &transport, &mut vault)?;
        skillet_workstation::ui_provisioning::verify_public_joplin(
            &transport,
            &sites,
            std::time::Duration::from_secs(120),
            std::time::Duration::from_secs(2),
        )?;
        provision_vm_airtrail(args, &profile, policy, &transport, &mut vault)?;
        provision_vm_media(args, &profile, policy, &transport, &mut vault)?;
    }
    if args.with_ddns {
        provision_vm_ddns(args, &run_dir, &transport, &mut vault)?;
    }
    if args.with_datadog {
        deliver_datadog_from_vault(
            &args.hostname,
            policy,
            &vault,
            &transport,
            skillet_vm::credential::ActivationPolicy::StartConsumer,
        )?;
    }
    if profile.supports_service("smtp") {
        let input = skillet_workstation::smtp_provisioning::input(policy, &|_, _| {
            Err(skillet_workstation::vault::VaultError::Invalid(
                "test policy must not read credentials".into(),
            ))
        })?;
        skillet_workstation::smtp_provisioning::deliver(
            &args.hostname,
            &input,
            &transport,
            skillet_vm::credential::ActivationPolicy::StartConsumer,
        )?;
    }
    provisioning_state::remove_tailscale_pending(&run_dir)?;
    println!("Tailscale connected VM {expected_hostname}");
    Ok(())
}

fn provision_vm_joplin(
    args: &VmProvisionArgs,
    profile: &skillet_hosts::HostProfile,
    policy: ProvisioningPolicy,
    transport: &impl GuestTransport,
    vault: &mut Vault,
) -> Result<()> {
    if !profile.supports_service("joplin") {
        return Ok(());
    }
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::joplin_provisioning::deliver_disposable(
        &args.hostname,
        &args.instance,
        policy,
        &mut store,
        transport,
    )?;
    skillet_workstation::host_apply::activate(transport, "skillet-joplin-apply.service")?;
    Ok(())
}

fn provision_vm_media(
    args: &super::VmProvisionArgs,
    profile: &skillet_hosts::HostProfile,
    policy: skillet_workstation::provisioning_policy::ProvisioningPolicy,
    transport: &impl GuestTransport,
    vault: &mut Vault,
) -> Result<()> {
    if !profile.supports_service("media") {
        return Ok(());
    }
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::media_provisioning::deliver(
        &args.hostname,
        Some(&args.instance),
        policy,
        &mut store,
        transport,
        skillet_vm::credential::ActivationPolicy::DeferConsumer,
    )?;
    skillet_workstation::host_apply::activate(transport, "skillet-media-apply.service")?;
    Ok(())
}

fn provision_vm_airtrail(
    args: &VmProvisionArgs,
    profile: &skillet_hosts::HostProfile,
    policy: ProvisioningPolicy,
    transport: &impl GuestTransport,
    vault: &mut Vault,
) -> Result<()> {
    if !profile.supports_service("airtrail") {
        return Ok(());
    }
    let mut store =
        skillet_workstation::vault::VaultSecretStore::new(vault, args.key_file.as_deref());
    skillet_workstation::airtrail_provisioning::deliver_disposable(
        &args.hostname,
        &args.instance,
        policy,
        &mut store,
        transport,
    )?;
    skillet_workstation::host_apply::activate(transport, "skillet-airtrail-apply.service")?;
    Ok(())
}

fn provision_vm_syncthing(
    args: &VmProvisionArgs,
    runtime_hostname: &str,
    vault: &Vault,
    transport: &impl GuestTransport,
) -> Result<()> {
    let profile = skillet_hosts::profile_for_name(&args.hostname)
        .ok_or_else(|| anyhow!("unknown Syncthing host"))?;
    if !profile.supports_service("syncthing") {
        return Ok(());
    }
    // New bootstrap units must converge before credential installation.
    skillet_workstation::host_apply::activate(transport, "skillet-apply.service")?;
    let policy = UiEnvironmentName::Test.policy();
    let activation = skillet_vm::credential::ActivationPolicy::StartConsumer;
    if args.with_ui || args.with_ddns {
        skillet_workstation::syncthing_provisioning::deliver(
            &args.hostname,
            runtime_hostname,
            policy,
            vault,
            transport,
            activation,
        )?;
    } else {
        let input = skillet_workstation::syncthing_provisioning::settings(
            &args.hostname,
            runtime_hostname,
            policy,
            "example.invalid",
        )?;
        skillet_workstation::syncthing_provisioning::deliver_settings(
            &input, transport, activation,
        )?;
    }
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
) -> Result<skillet_caddy::CaddySites> {
    let users = tailnet_ui_users(&args.hostname, vault)?;
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
            public_targets: &args.public_target,
            host: &args.hostname,
            instance: &args.instance,
            policy,
            zone_id: &zone_id,
            relative_ui_domain: domain_prefix.as_deref(),
            tailnet_users: &users,
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
    Ok(provisioned.sites)
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
    let config = skillet_workstation::configuration_templates::render(
        "ddns",
        &args.hostname,
        policy.vault_name(),
        vault,
    )?;
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

fn provision_vm_smtp(args: &VmProvisionArgs, transport: &impl GuestTransport) -> Result<()> {
    let input = if args.live_smtp {
        let vault_path = args
            .database
            .clone()
            .map_or_else(default_database_path, Ok)?;
        let vault = Vault::open(&vault_path, args.key_file.as_deref())?;
        skillet_workstation::smtp_provisioning::input(
            UiEnvironmentName::Production.policy(),
            &|path, field| vault.get_field(path, field),
        )?
    } else {
        skillet_workstation::smtp_provisioning::input(UiEnvironmentName::Test.policy(), &|_, _| {
            Err(skillet_workstation::vault::VaultError::Invalid(
                "test policy must not read credentials".into(),
            ))
        })?
    };
    skillet_workstation::smtp_provisioning::deliver(
        &args.hostname,
        &input,
        transport,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )?;
    if args.live_smtp {
        println!("Configured live SMTP provider; submission remains explicit and loopback-only");
    } else {
        println!("Configured isolated SMTP capture relay; start a loopback sink on port 1025");
    }
    Ok(())
}

#[cfg(test)]
#[path = "secret_delivery/tests.rs"]
mod tests;

fn tailnet_ui_users(host: &str, vault: &Vault) -> Result<Vec<String>> {
    let profile =
        skillet_hosts::profile_for_name(host).ok_or_else(|| anyhow!("unknown host profile"))?;
    if !profile.ui_services().iter().any(|ui| ui.tailnet_identity) {
        return Ok(Vec::new());
    }
    let users = vault.get(skillet_caddy::auth::USERS_ENTRY)?.ok_or_else(||
        anyhow!("KeePassXC entry is missing: {}; put full LoginName values in Password, separated by commas or newlines", skillet_caddy::auth::USERS_ENTRY))?;
    Ok(skillet_caddy::auth::Policy::parse_users(&users)?)
}
