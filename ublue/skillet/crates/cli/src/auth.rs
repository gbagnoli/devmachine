use anyhow::Result;
use skillet_workstation::{
    credential_cache::{CredentialCache as _, KernelCache},
    sudo::SUDO_CACHE_NAMESPACE,
};

#[derive(clap::Subcommand, Debug)]
pub(super) enum Commands {
    /// Forget cached sudo passwords; leave vault and SSH-agent unlocks unchanged
    Lock,
}
pub(super) fn dispatch(command: &Commands) -> Result<()> {
    match command {
        Commands::Lock => KernelCache::new(SUDO_CACHE_NAMESPACE).clear()?,
    }
    println!("Cleared cached sudo passwords");
    Ok(())
}
#[cfg(test)]
#[path = "auth/tests.rs"]
mod tests;
