use anyhow::Result;
use skillet_cli_common::run_host;

fn main() -> Result<()> {
    run_host("clamps")?;
    Ok(())
}
