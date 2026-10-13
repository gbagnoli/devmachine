use anyhow::{Context, Result};
use skillet_vm::transport::GuestTransport;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub(super) struct Args {
    #[arg(long)]
    host: String,
    #[arg(long)]
    target: String,
    #[arg(long, default_value_t = 22)]
    port: u16,
    #[arg(long)]
    identity: Option<PathBuf>,
    #[arg(long)]
    known_hosts: Option<PathBuf>,
}

pub(super) fn dispatch(args: &Args) -> Result<()> {
    skillet_hosts::profile_for_name(&args.host)
        .ok_or_else(|| anyhow::anyhow!("unknown host profile: {}", args.host))?;
    let transport = super::ssh::transport(
        &args.target,
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
