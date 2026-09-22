use anyhow::Result;
use skillet_cli_common::{hosts, run_host};
use skillet_core::credentials::CredentialManager;

fn main() -> Result<()> {
    run_host("clamps", |system, files| {
        // Initialize credential manager once
        let cred_manager = CredentialManager::new().map_err(|e| e.to_string())?;
        hosts::apply_clamps(system, files, &cred_manager).map_err(|e| e.to_string())
    })?;
    Ok(())
}
