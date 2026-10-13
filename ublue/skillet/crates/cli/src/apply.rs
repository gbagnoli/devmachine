use super::{SecretDeliverArgs, UiEnvironmentName};
use anyhow::{anyhow, Context, Result};
use skillet_cli_common::hosts::ApplyPhase;
use skillet_vm::{credential::ActivationPolicy, transport::SshTransport};
use skillet_workstation::{host_apply, vault::Vault};
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub(super) struct Args {
    #[arg(long, value_enum, default_value_t = ApplyPhase::Full)]
    phase: ApplyPhase,
    #[arg(long)]
    host: Option<String>,
    #[arg(long, conflicts_with = "target")]
    host_file: Option<PathBuf>,
    /// Record local guest resource operations
    #[arg(long, conflicts_with = "target")]
    record: Option<PathBuf>,
    /// Provision all declared services over verified SSH to USER@HOST
    #[arg(long, requires = "host")]
    target: Option<String>,
    /// Reuse installed guest binaries instead of deploying current source
    #[arg(long, requires = "target")]
    no_deploy: bool,
    #[arg(long, default_value_t = 22)]
    port: u16,
    /// SSH private key; omitted uses OpenSSH configuration, defaults and agent
    #[arg(long, requires = "target")]
    identity: Option<PathBuf>,
    /// Recorded host keys; defaults to `~/.ssh/known_hosts`
    #[arg(long, requires = "target")]
    known_hosts: Option<PathBuf>,
    #[arg(long, requires = "target")]
    database: Option<PathBuf>,
    #[arg(long, requires = "target")]
    key_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = UiEnvironmentName::Production)]
    environment: UiEnvironmentName,
}

pub(super) fn dispatch(args: &Args) -> Result<()> {
    if args.target.is_some() {
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
        .ok_or_else(|| anyhow!("remote apply requires --host"))?;
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| anyhow!("unknown host profile: {host}"))?;
    let target = args
        .target
        .as_deref()
        .ok_or_else(|| anyhow!("remote apply requires --target"))?;
    let transport = super::ssh::transport(
        target,
        args.port,
        args.identity.as_deref(),
        args.known_hosts.as_deref(),
    )?;
    skillet_workstation::deployment::verify_host(host, &transport)?;
    if args.no_deploy {
        println!(
            "Workstation Skillet {}; deployment skipped (--no-deploy)",
            skillet_cli_common::version::version()
        );
        let installed = skillet_workstation::deployment::installed_versions(host, &transport)?;
        println!("Installed: {}", installed.generic_version);
        println!("Installed: {}", installed.host_version);
    } else {
        super::deploy::deploy(host, &transport)?;
    }
    let path = args.database.clone().map_or_else(Vault::default_path, Ok)?;
    let mut vault = Vault::open(&path, args.key_file.as_deref())?;
    let report = skillet_workstation::secrets::check(
        std::slice::from_ref(&profile),
        args.environment.policy(),
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
        hostname: host.into(),
        service: String::new(),
        database: Some(path),
        key_file: args.key_file.clone(),
        environment: args.environment,
        target: target.into(),
        port: args.port,
        identity: args.identity.clone(),
        known_hosts: args.known_hosts.clone(),
        activate: false,
    };
    host_apply::run(
        &profile,
        &mut Runner {
            delivery,
            vault: &mut vault,
            transport: &transport,
            wait_for_tailscale: profile.supports_service("tailscale"),
        },
    )?;
    println!("Applied all declared services for {host}");
    if profile.host_network_selection().is_some() {
        println!("Network profiles prepared for the next boot; live connections were not switched");
    }
    Ok(())
}

struct Runner<'a> {
    delivery: SecretDeliverArgs,
    vault: &'a mut Vault,
    transport: &'a SshTransport,
    wait_for_tailscale: bool,
}

impl host_apply::Actions for Runner<'_> {
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
            .with_context(|| format!("staging {service}; previously completed steps are retained, retry apply after resolving the failure"))
    }

    fn activate(&mut self, unit: &'static str) -> Result<()> {
        println!("Applying {unit}");
        host_apply::activate(self.transport, unit).with_context(|| format!("applying {unit}"))?;
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
