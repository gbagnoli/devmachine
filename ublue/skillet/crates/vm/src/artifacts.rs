//! Select binaries only from the successful Cargo invocation's JSON output.
use crate::{Error, Result};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

pub struct BuildRequest<'a> {
    pub workspace: &'a Path,
    pub packages: &'a [&'a str],
    pub target: &'a str,
    pub profile: &'a str,
}

pub fn build(request: &BuildRequest<'_>) -> Result<BTreeMap<String, PathBuf>> {
    let mut command = Command::new("cargo");
    command.current_dir(request.workspace).args([
        "build",
        "--message-format=json-render-diagnostics",
        "--target",
        request.target,
        "--profile",
        request.profile,
    ]);
    for package in request.packages {
        command.args(["--package", package]);
    }
    let output = crate::process::capture(command, std::time::Duration::from_mins(30))?;
    if !output.status.success() {
        // Compiler diagnostics contain source details, not credential payloads.
        // Return the rendered diagnostic without dumping arbitrary tool output.
        let rendered = diagnostics(&output.stdout);
        let diagnostic = if rendered.is_empty() {
            String::from_utf8_lossy(&output.stderr).into_owned()
        } else {
            rendered
        };
        return Err(Error::Build {
            code: output.status.code(),
            diagnostic,
        });
    }
    let artifacts = reported_artifacts(&output.stdout, request.packages, request.workspace)?;
    artifacts
        .into_iter()
        .map(|(name, path)| {
            if !path.is_file() {
                return Err(Error::Invalid("Cargo-reported executable is absent".into()));
            }
            Ok((name, std::fs::canonicalize(path)?))
        })
        .collect()
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    target: Option<CargoTarget>,
    executable: Option<PathBuf>,
}
#[derive(Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

pub fn reported_artifacts(
    output: &[u8],
    packages: &[&str],
    workspace: &Path,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut artifacts = BTreeMap::new();
    for line in output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: CargoMessage = serde_json::from_slice(line)?;
        if message.reason != "compiler-artifact" {
            continue;
        }
        if let (Some(target), Some(path)) = (message.target, message.executable) {
            if target.kind.iter().any(|kind| kind == "bin")
                && packages.contains(&target.name.as_str())
            {
                let path = if path.is_absolute() {
                    path
                } else {
                    workspace.join(path)
                };
                if artifacts.insert(target.name, path).is_some() {
                    return Err(Error::Invalid(
                        "multiple Cargo executables match one requested binary".into(),
                    ));
                }
            }
        }
    }
    if packages
        .iter()
        .any(|package| !artifacts.contains_key(*package))
    {
        return Err(Error::Invalid(
            "successful Cargo build did not report every requested executable".into(),
        ));
    }
    Ok(artifacts)
}

fn diagnostics(output: &[u8]) -> String {
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .filter_map(|message| {
            message
                .get("message")
                .and_then(|message| message.get("rendered"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "artifacts/tests.rs"]
mod tests;
