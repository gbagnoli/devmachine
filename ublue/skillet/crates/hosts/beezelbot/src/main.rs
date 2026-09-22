use anyhow::Result;
use skillet_cli_common::{hosts, run_host};

fn main() -> Result<()> {
    run_host("beezelbot", |system, files| {
        hosts::apply_beezelbot(system, files).map_err(|e| e.to_string())
    })?;
    Ok(())
}
