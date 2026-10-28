//! Regroup an existing generated dashboard from stdin using its host caller.
//! Output may contain widget credentials: keep redirected files private.
use std::io::{Read as _, Write as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::args()
        .nth(1)
        .ok_or("usage: dashboard_groups HOST < existing.yml > grouped.yml")?;
    let profile = skillet_hosts::profile_for_name(&host).ok_or("unknown host")?;
    let mut input = String::new();
    std::io::stdin()
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 4 * 1024 * 1024 {
        return Err("dashboard input is too large".into());
    }
    let output = skillet_homer::regroup(&input, |name| profile.dashboard_group(name))?;
    std::io::stdout().write_all(output.as_bytes())?;
    Ok(())
}
