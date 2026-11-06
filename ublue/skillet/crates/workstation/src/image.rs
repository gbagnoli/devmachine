//! Public image bundle assembly; never invokes guest apply or reads a vault.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io, os::unix::fs::PermissionsExt as _, path::Path};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown image host")]
    Host,
    #[error("image bundles require a full source commit and matching clean binary versions")]
    Revision,
    #[error("image binaries must be static little-endian x86-64 ELF executables")]
    Executable,
    #[error("output already exists with different content, metadata, or extra files")]
    Conflict,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Artifact(#[from] skillet_vm::Error),
}

#[derive(Serialize)]
struct Release<'a> {
    format_version: u32,
    host: &'a str,
    source_commit: &'a str,
    version: &'a str,
    services: Vec<&'a str>,
    artifacts: Vec<Receipt>,
}

#[derive(Serialize)]
struct Receipt {
    path: String,
    sha256: String,
    mode: u32,
}

struct Output {
    path: String,
    bytes: Vec<u8>,
    mode: u32,
}

pub fn export(
    host: &str,
    revision: &str,
    generic: &Path,
    caller: &Path,
    output: &Path,
) -> Result<(), Error> {
    let generic_version = skillet_vm::artifacts::binary_version(generic)?;
    let caller_version = skillet_vm::artifacts::binary_version(caller)?;
    let version = generic_version
        .strip_prefix("skillet ")
        .ok_or(Error::Revision)?;
    if caller_version != format!("skillet-{host} {version}") {
        return Err(Error::Revision);
    }
    let files = render(
        host,
        revision,
        version,
        &fs::read(generic)?,
        &fs::read(caller)?,
    )?;
    write(output, &files)
}

fn render(
    host: &str,
    revision: &str,
    version: &str,
    generic: &[u8],
    caller: &[u8],
) -> Result<Vec<Output>, Error> {
    static_elf(generic)?;
    static_elf(caller)?;
    let profile = skillet_hosts::profile_for_name(host)
        .filter(|p| p.signed_image.is_some())
        .ok_or(Error::Host)?;
    if revision.len() != 40
        || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        || !version.ends_with(&format!("+git.{}", &revision[..12]))
    {
        return Err(Error::Revision);
    }
    let mut files = vec![
        Output {
            path: "usr/bin/skillet".into(),
            bytes: generic.into(),
            mode: 0o755,
        },
        Output {
            path: format!("usr/bin/skillet-{host}"),
            bytes: caller.into(),
            mode: 0o755,
        },
    ];
    let release = Release {
        format_version: 1,
        host,
        source_commit: revision,
        version,
        services: profile
            .services
            .iter()
            .map(skillet_hosts::HostService::name)
            .collect(),
        artifacts: files
            .iter()
            .map(|f| Receipt {
                path: format!("/{}", f.path),
                sha256: hex::encode(Sha256::digest(&f.bytes)),
                mode: f.mode,
            })
            .collect(),
    };
    files.push(Output {
        path: "usr/share/skillet/release.json".into(),
        bytes: serde_json::to_vec_pretty(&release)?,
        mode: 0o644,
    });
    Ok(files)
}

fn write(output: &Path, files: &[Output]) -> Result<(), Error> {
    if output.try_exists()? {
        let mut found = Vec::new();
        inventory(output, output, &mut found)?;
        let mut expected: Vec<_> = files.iter().map(|f| f.path.clone()).collect();
        found.sort();
        expected.sort();
        if found != expected {
            return Err(Error::Conflict);
        }
        for file in files {
            let path = output.join(&file.path);
            if fs::read(&path)? != file.bytes
                || fs::metadata(path)?.permissions().mode() & 0o7777 != file.mode
            {
                return Err(Error::Conflict);
            }
        }
        return Ok(());
    }
    let parent = fs::canonicalize(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let staging = tempfile::tempdir_in(parent)?;
    for file in files {
        let path = staging.path().join(&file.path);
        fs::create_dir_all(path.parent().ok_or(Error::Conflict)?)?;
        let mut directory = path.parent();
        while let Some(current) = directory.filter(|p| p.starts_with(staging.path())) {
            fs::set_permissions(current, fs::Permissions::from_mode(0o755))?;
            directory = current.parent();
        }
        fs::write(&path, &file.bytes)?;
        fs::set_permissions(path, fs::Permissions::from_mode(file.mode))?;
    }
    fs::rename(staging.path(), output)?;
    Ok(())
}

fn inventory(root: &Path, path: &Path, found: &mut Vec<String>) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        return Err(Error::Conflict);
    }
    if metadata.is_dir() {
        if metadata.permissions().mode() & 0o7777 != 0o755 {
            return Err(Error::Conflict);
        }
        for entry in fs::read_dir(path)? {
            inventory(root, &entry?.path(), found)?;
        }
    } else if metadata.is_file() {
        found.push(
            path.strip_prefix(root)
                .map_err(|_| Error::Conflict)?
                .to_string_lossy()
                .into(),
        );
    } else {
        return Err(Error::Conflict);
    }
    Ok(())
}

#[cfg(test)]
#[path = "image/tests.rs"]
mod tests;

fn static_elf(bytes: &[u8]) -> Result<(), Error> {
    if bytes.get(..6) != Some(b"\x7fELF\x02\x01") || bytes.get(18..20) != Some(&62u16.to_le_bytes())
    {
        return Err(Error::Executable);
    }
    let offset = usize::try_from(u64::from_le_bytes(
        bytes
            .get(32..40)
            .ok_or(Error::Executable)?
            .try_into()
            .map_err(|_| Error::Executable)?,
    ))
    .map_err(|_| Error::Executable)?;
    let size = u16::from_le_bytes(
        bytes
            .get(54..56)
            .ok_or(Error::Executable)?
            .try_into()
            .map_err(|_| Error::Executable)?,
    );
    let count = u16::from_le_bytes(
        bytes
            .get(56..58)
            .ok_or(Error::Executable)?
            .try_into()
            .map_err(|_| Error::Executable)?,
    );
    if size != 56 || count == 0 {
        return Err(Error::Executable);
    }
    for index in 0..usize::from(count) {
        let start = offset.checked_add(index * 56).ok_or(Error::Executable)?;
        let entry = bytes
            .get(start..start.checked_add(56).ok_or(Error::Executable)?)
            .ok_or(Error::Executable)?;
        if entry[..4] == 3u32.to_le_bytes() {
            return Err(Error::Executable);
        }
    }
    Ok(())
}
