use anyhow::Result;
use skillet_cli_common::run_host;

fn main() -> Result<()> {
    run_host("beezelbot", |system, files| {
        skillet_hardening::apply(system, files).map_err(|e| e.to_string())
    })?;
    Ok(())
}
