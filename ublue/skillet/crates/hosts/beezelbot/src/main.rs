use anyhow::Result;
use skillet_cli_common::{hosts, run_host};

fn main() -> Result<()> {
    run_host("beezelbot", |phase, system, files| {
        hosts::apply_host_phase("beezelbot", phase, system, files).map_err(|e| e.to_string())
    })?;
    Ok(())
}
