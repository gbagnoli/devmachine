use anyhow::Context as _;

fn main() -> anyhow::Result<()> {
    skillet_cli_common::handle_apply("smoke fixture", None, |system, files| {
        skillet_smoke_fixture::apply(system, files).map_err(|error| error.to_string())
    })
    .context("applying smoke fixture failed")?;
    Ok(())
}
