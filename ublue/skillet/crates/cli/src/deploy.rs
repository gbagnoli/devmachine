use anyhow::{Context, Result};
use skillet_vm::transport::GuestTransport;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub(super) struct Args {
    /// Canonical host profile and default SSH alias
    host: String,
    /// Override SSH destination: HOST, alias, or USER@HOST
    #[arg(long)]
    target: Option<String>,
    /// Override SSH config port
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    identity: Option<PathBuf>,
    #[arg(long)]
    known_hosts: Option<PathBuf>,
}

pub(super) fn dispatch(args: &Args) -> Result<()> {
    skillet_hosts::profile_for_name(&args.host)
        .ok_or_else(|| anyhow::anyhow!("unknown host profile: {}", args.host))?;
    let transport = super::ssh::transport(
        args.target.as_deref().unwrap_or(&args.host),
        args.port,
        args.identity.as_deref(),
        args.known_hosts.as_deref(),
    )?;
    deploy(&args.host, &transport)
}

pub(super) fn deploy(host: &str, guest: &impl GuestTransport) -> Result<()> {
    println!(
        "Workstation Skillet {}",
        skillet_cli_common::version::version()
    );
    println!("Building and deploying skillet and skillet-{host}");
    let report = skillet_workstation::deployment::deploy(host, &super::workspace_root()?, guest)
        .context(
        "deploying Skillet; apply has not started, retry deployment after resolving the failure",
    )?;
    println!("Deployed and verified: {}", report.generic_version);
    println!("Deployed and verified: {}", report.host_version);
    Ok(())
}
