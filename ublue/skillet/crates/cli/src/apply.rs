use super::{SecretDeliverArgs, UiEnvironmentName};
use anyhow::{anyhow, Context, Result};
use skillet_cli_common::hosts::ApplyPhase;
use skillet_vm::{credential::ActivationPolicy, transport::GuestTransport};
use skillet_workstation::{host_apply, vault::Vault};
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
#[command(after_help = include_str!("help/ssh-agent.txt"))]
#[command(
    override_usage = "skillet apply [OPTIONS] <HOST>\n       skillet apply --host-file <PATH> [OPTIONS]"
)]
#[command(group(clap::ArgGroup::new("local_mode").args(["host_file", "local"]).multiple(false)))]
pub(super) struct Args {
    /// Public ingress IP or existing DNS hostname; repeat for IPv4 and IPv6
    #[arg(long, conflicts_with = "local_mode")]
    public_target: Vec<String>,
    #[arg(long, value_enum, default_value_t = ApplyPhase::Full)]
    phase: ApplyPhase,
    /// Required host profile and default SSH alias (except with --host-file)
    #[arg(required_unless_present = "host_file", conflicts_with = "host_file")]
    host: Option<String>,
    #[arg(long, conflicts_with_all = ["target", "local", "no_deploy", "identity", "known_hosts", "database", "key_file", "port"])]
    host_file: Option<PathBuf>,
    /// Apply on this guest rather than connecting over SSH
    #[arg(long, conflicts_with_all = ["target", "no_deploy", "identity", "known_hosts", "database", "key_file", "port"])]
    local: bool,
    /// Record local guest resource operations
    #[arg(long, requires = "local_mode")]
    record: Option<PathBuf>,
    /// Override SSH destination: HOST, alias, or USER@HOST
    #[arg(long)]
    target: Option<String>,
    /// Use only the active image binaries; skip temporary workstation runners
    #[arg(long, conflicts_with_all = ["host_file", "local"])]
    no_deploy: bool,
    /// Override SSH config port
    #[arg(long)]
    port: Option<u16>,
    /// SSH private key; omitted uses OpenSSH configuration, defaults and agent
    #[arg(long)]
    identity: Option<PathBuf>,
    /// Override recorded host keys; omitted uses OpenSSH settings
    #[arg(long)]
    known_hosts: Option<PathBuf>,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    key_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = UiEnvironmentName::Production)]
    environment: UiEnvironmentName,
}

pub(super) fn dispatch(args: &Args) -> Result<()> {
    if !args.local && args.host_file.is_none() {
        return remote(args);
    }
    let mut hostname = args.host.clone().unwrap_or_else(|| "agent".into());
    if let Some(path) = &args.host_file {
        hostname = std::fs::read_to_string(path)
            .context("Failed to read host file")?
            .trim()
            .into();
    }
    skillet_cli_common::handle_host_apply(
        &hostname,
        args.phase,
        args.record.clone(),
        |system, files, credentials| {
            skillet_cli_common::hosts::apply_host_phase(
                &hostname,
                args.phase,
                system,
                files,
                credentials,
            )
            .map_err(|error| error.to_string())
        },
    )
    .map_err(|error| anyhow!("Failed to apply configuration: {error}"))
}

fn remote(args: &Args) -> Result<()> {
    if args.phase != ApplyPhase::Full || !matches!(args.environment, UiEnvironmentName::Production)
    {
        return Err(anyhow!("remote apply provisions all production services; test provisioning requires an owned disposable VM"));
    }
    let host = args
        .host
        .as_deref()
        .ok_or_else(|| anyhow!("remote apply requires HOST"))?;
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| anyhow!("unknown host profile: {host}"))?;
    skillet_workstation::ui_provisioning::validate_public_targets(
        host,
        &args.public_target,
        args.environment.policy(),
    )?;
    let target = args.target.as_deref().unwrap_or(host);
    let transport = super::ssh::transport(
        target,
        args.port,
        args.identity.as_deref(),
        args.known_hosts.as_deref(),
    )?;
    skillet_workstation::deployment::verify_host(host, &transport)?;
    let installed = skillet_workstation::deployment::installed_versions(host, &transport)?;
    println!("Image-owned guest binaries:");
    println!("  {}", installed.generic_version);
    println!("  {}", installed.host_version);
    let path = args.database.clone().map_or_else(Vault::default_path, Ok)?;
    let mut vault = Vault::open(&path, args.key_file.as_deref())?;
    let report = skillet_workstation::secrets::check_with_entries(
        std::slice::from_ref(&profile),
        args.environment.policy(),
        &vault.entry_paths(),
        &|path, field| vault.get_field(path, field),
    )?;
    if !report.missing.is_empty() {
        print!("{}", super::secret_output::render(&report, &[], false));
        return Err(anyhow!(
            "required vault entries need attention; no services applied"
        ));
    }
    if profile.host_network_selection().is_some() {
        // Validate subnet relationships before applying any service, not just field formats.
        skillet_workstation::network_provisioning::input(
            host,
            args.environment.policy(),
            &|path, field| vault.get_field(path, field),
        )?;
    }
    let delivery = SecretDeliverArgs {
        public_target: args.public_target.clone(),
        hostname: host.into(),
        service: String::new(),
        database: Some(path),
        key_file: args.key_file.clone(),
        environment: args.environment,
        target: Some(target.into()),
        port: args.port,
        identity: args.identity.clone(),
        known_hosts: args.known_hosts.clone(),
        activate: false,
    };
    apply_remote_services(
        host, args, &profile, &installed, delivery, &mut vault, &transport,
    )
}

fn apply_remote_services<T: GuestTransport>(
    host: &str,
    args: &Args,
    profile: &skillet_hosts::HostProfile,
    installed: &skillet_workstation::deployment::Report,
    delivery: SecretDeliverArgs,
    vault: &mut Vault,
    transport: &T,
) -> Result<()> {
    let temporary_runner = if args.no_deploy {
        println!("Using the active image binaries (--no-deploy)");
        None
    } else {
        println!("Building and staging a temporary remote apply runner");
        Some(skillet_workstation::deployment::stage_temporary_runner(
            host,
            &super::workspace_root()?,
            transport,
        )?)
    };
    if let Some(runner) = &temporary_runner {
        println!("Temporary workstation runner: {}", runner.version());
        if runner.version() != installed.generic_version {
            eprintln!(
                "These versions differ. This command uses the workstation runner only for the current apply. After cleanup, systemd uses the image's /usr/bin/skillet; update the image before relying on newer code after reboot."
            );
        }
    }
    let apply_result = host_apply::run(
        profile,
        &mut Runner {
            delivery,
            vault,
            transport,
            temporary_binary: temporary_runner
                .as_ref()
                .map(|runner| runner.path().to_owned()),
            temporary_token: temporary_runner
                .as_ref()
                .map(|runner| runner.token().to_owned()),
            wait_for_tailscale: profile.supports_service("tailscale"),
        },
    );
    let cleanup_result = temporary_runner
        .as_ref()
        .map(|runner| skillet_workstation::deployment::cleanup_temporary_runner(runner, transport));
    apply_result?;
    if let Some(result) = cleanup_result {
        result?;
    }
    println!("Applied all declared services for {host}");
    if profile.host_network_selection().is_some() {
        println!("Network profiles prepared for the next boot; live connections were not switched");
    }
    Ok(())
}

struct Runner<'a, T: GuestTransport> {
    delivery: SecretDeliverArgs,
    vault: &'a mut Vault,
    transport: &'a T,
    temporary_binary: Option<String>,
    temporary_token: Option<String>,
    wait_for_tailscale: bool,
}

impl<T: GuestTransport> host_apply::Actions for Runner<'_, T> {
    type Error = anyhow::Error;

    fn stage(&mut self, service: &'static str) -> Result<()> {
        self.delivery.service = service.into();
        if service == "tailscale"
            && skillet_workstation::credential_delivery::tailscale_credential_present(
                &self.delivery.hostname,
                self.transport,
            )?
        {
            println!("Reusing installed Tailscale enrollment credential");
            return Ok(());
        }
        println!("Staging {service} credentials");
        super::secret_delivery::deliver_with_vault(&self.delivery, self.vault, self.transport, ActivationPolicy::DeferConsumer)
            .map(|_| ())
            .with_context(|| format!("staging {service}; previously completed steps are retained, retry apply after resolving the failure"))
    }

    fn activate(&mut self, unit: &'static str) -> Result<()> {
        println!("Applying {unit}");
        if let Some(token) = &self.temporary_token {
            let binary = self
                .temporary_binary
                .as_deref()
                .ok_or_else(|| anyhow!("temporary apply runner is incomplete"))?;
            let arguments = host_apply::temporary_arguments(unit)
                .ok_or_else(|| anyhow!("missing temporary command declaration for {unit}"))?;
            skillet_workstation::deployment::activate_with_runner(
                self.transport,
                unit,
                binary,
                token,
                arguments,
            )
            .with_context(|| format!("applying {unit} with the temporary runner"))?;
        } else {
            host_apply::activate(self.transport, unit)
                .with_context(|| format!("applying {unit}"))?;
        }
        if unit == "skillet-full-apply.service" && self.wait_for_tailscale {
            println!("Waiting for Tailscale enrollment before configuring UI DNS");
            skillet_workstation::tailscale_enrollment::wait_for_guest_addresses(
                self.transport,
                std::time::Duration::from_mins(2),
                std::time::Duration::from_secs(2),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "apply/tests.rs"]
mod tests;
