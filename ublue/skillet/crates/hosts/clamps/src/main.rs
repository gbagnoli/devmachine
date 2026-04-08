use anyhow::Result;
use skillet_cli_common::run_host;

fn main() -> Result<()> {
    run_host("clamps", |system, files| {
        skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;
        skillet_pihole::apply(
            system,
            files,
            skillet_pihole::PiholeUser {
                uid: 40000,
                gid: 40000,
                name: "pihole".to_string(),
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })?;
    Ok(())
}
