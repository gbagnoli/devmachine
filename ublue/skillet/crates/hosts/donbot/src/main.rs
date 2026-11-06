use anyhow::Result;
use skillet_cli_common::{hosts, run_host};

fn main() -> Result<()> {
    run_host("donbot", |phase, system, files, credentials| {
        hosts::apply_host_phase("donbot", phase, system, files, credentials)
            .map_err(|error| error.to_string())
    })?;
    Ok(())
}
