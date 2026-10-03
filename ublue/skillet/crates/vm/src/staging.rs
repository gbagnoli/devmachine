//! Stage a host's Butane inputs and captured binaries for one disposable run.
use crate::{manifest::reject_symlinks, Error, ManifestStore, Phase, Result, VmRun};
use serde_yml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::Path,
};

/// Copy and specialize the source Butane tree under the recorded run directory.
/// Repeated staging writes only changed bytes and never mutates source files.
pub fn stage_butane_source(
    store: &ManifestStore,
    run: &VmRun,
    config: &Path,
    includes: &Path,
    host_binary: &Path,
    generic_binary: &Path,
    image: &Path,
) -> Result<()> {
    let _lock = store.lock(&run.identity)?;
    let current = store.load(&run.identity)?;
    store.validate(&current, &run.identity)?;
    if current.phase != Phase::Preparing {
        return Err(Error::Invalid(
            "Butane staging is allowed only for a preparing VM".into(),
        ));
    }
    let run = &current;
    for path in [config, includes, host_binary, generic_binary, image] {
        reject_symlinks(path)?;
    }
    if !config.is_file()
        || !includes.is_dir()
        || !host_binary.is_file()
        || !generic_binary.is_file()
        || !image.is_file()
    {
        return Err(Error::Invalid(
            "Butane staging inputs must be regular files and an include directory".into(),
        ));
    }
    let run_dir = store.run_dir(&run.identity);
    reject_symlinks(&run_dir)?;
    let source_dir = run_dir.join("source");
    let include_dir = source_dir.join("includes");
    let files_dir = source_dir.join("files");
    create_private_dir(&source_dir)?;
    create_private_dir(&include_dir)?;
    create_private_dir(&files_dir)?;
    stage_config(run, config, includes, &source_dir, &include_dir)?;
    stage_artifacts(
        run,
        host_binary,
        generic_binary,
        image,
        &files_dir,
        &run_dir,
    )?;
    File::open(&run_dir)?.sync_all()?;
    Ok(())
}

fn stage_config(
    run: &VmRun,
    config: &Path,
    includes: &Path,
    source_dir: &Path,
    include_dir: &Path,
) -> Result<()> {
    let config_target = source_dir.join(format!("{}.bu", run.identity.host()));
    let mut host_config = parse_yaml(config)?;
    set_inline(
        find_file(&mut host_config, "/etc/hostname")?,
        &run.guest_hostname,
    )?;
    write_yaml_if_changed(&config_target, &host_config)?;

    for entry in fs::read_dir(includes)? {
        let entry = entry?;
        let source = entry.path();
        if source
            .extension()
            .is_some_and(|extension| extension == "bu")
        {
            reject_symlinks(&source)?;
            if !entry.file_type()?.is_file() {
                return Err(Error::Invalid(
                    "Butane include must be a regular file".into(),
                ));
            }
            let target = include_dir.join(entry.file_name());
            copy_if_changed(&source, &target, 0o644)?;
        }
    }

    let shared_include = include_dir.join("skillet.bu");
    let mut shared = parse_yaml(&shared_include)?;
    remove_file(&mut shared, "/var/usrlocal/bin/skillet")?;
    write_yaml_if_changed(&shared_include, &shared)?;

    let host_include = include_dir.join(format!("skillet-{}.bu", run.identity.host()));
    let mut host_specific = parse_yaml(&host_include)?;
    remove_file(
        &mut host_specific,
        &format!("/var/usrlocal/bin/skillet-{}", run.identity.host()),
    )?;
    write_yaml_if_changed(&host_include, &host_specific)?;

    let passwd_include = include_dir.join("passwd.bu");
    let mut passwd = parse_yaml(&passwd_include)?;
    add_authorized_key(
        &mut passwd,
        &crate::provisioning::validated_public_key(run)?,
    )?;
    remove_file(&mut passwd, "/usr/local/bin/force_pw_change.sh")?;
    remove_unit(&mut passwd, "force-pw-change.service")?;
    add_vm_sudoers(&mut passwd)?;
    write_yaml_if_changed(&passwd_include, &passwd)?;
    Ok(())
}

fn stage_artifacts(
    run: &VmRun,
    host_binary: &Path,
    generic_binary: &Path,
    image: &Path,
    files_dir: &Path,
    run_dir: &Path,
) -> Result<()> {
    let host_staged = files_dir.join(format!("skillet-{}", run.identity.host()));
    let generic_staged = files_dir.join("skillet");
    copy_if_changed(host_binary, &host_staged, 0o755)?;
    copy_if_changed(generic_binary, &generic_staged, 0o755)?;
    let host_hash = hash_path(&host_staged)?;
    let generic_hash = hash_path(&generic_staged)?;
    let image_name = image
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::Invalid("VM image filename is invalid".into()))?;
    let image_hash = hash_path(image)?;
    let image_sidecar = run_dir.join("fcos-image.sha256");
    write_if_changed(
        &image_sidecar,
        format!("{image_hash}  {image_name}\n").as_bytes(),
        0o600,
    )?;
    write_if_changed(
        &run_dir.join("skillet.sha256"),
        format!("{host_hash}  {}\n", host_staged.display()).as_bytes(),
        0o600,
    )?;
    write_if_changed(
        &run_dir.join("skillet-generic.sha256"),
        format!("{generic_hash}  {}\n", generic_staged.display()).as_bytes(),
        0o600,
    )?;
    write_if_changed(
        &run_dir.join("run.conf"),
        legacy_config(run, image, host_binary)?.as_bytes(),
        0o600,
    )?;
    Ok(())
}

fn legacy_config(run: &VmRun, image: &Path, host_binary: &Path) -> Result<String> {
    let fields = [
        ("vm", run.identity.domain_name()),
        ("host", run.identity.host().to_owned()),
        ("uri", run.connection.uri.clone()),
        ("ssh_port", run.ssh.port.to_string()),
        ("image", path_value(image)?),
        ("artifact", path_value(host_binary)?),
        ("runtime_dir", path_value(&run.connection.runtime_dir)?),
        (
            "backend",
            match run.connection.backend {
                crate::Backend::Native => "native",
                crate::Backend::Flatpak => "flatpak",
            }
            .to_owned(),
        ),
        ("source_commit", run.source_commit.clone()),
    ];
    if fields.iter().any(|(_, value)| value.contains(['\n', '\r'])) {
        return Err(Error::Invalid(
            "newline in legacy VM configuration value".into(),
        ));
    }
    let mut output = String::new();
    for (key, value) in fields {
        output.push_str(key);
        output.push('=');
        output.push_str(&value);
        output.push('\n');
    }
    Ok(output)
}

fn path_value(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| Error::Invalid("VM artifact path is not UTF-8".into()))
}

fn parse_yaml(path: &Path) -> Result<Value> {
    reject_symlinks(path)?;
    let contents = fs::read_to_string(path)?;
    serde_yml::from_str(&contents).map_err(|error| Error::Preparation(error.to_string()))
}

fn write_yaml_if_changed(path: &Path, value: &Value) -> Result<()> {
    let contents =
        serde_yml::to_string(value).map_err(|error| Error::Preparation(error.to_string()))?;
    write_if_changed(path, contents.as_bytes(), 0o644)
}

fn create_private_dir(path: &Path) -> Result<()> {
    reject_symlinks(path)?;
    if !path.exists() {
        fs::create_dir(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    if !fs::metadata(path)?.is_dir() {
        return Err(Error::Invalid("VM staging path is not a directory".into()));
    }
    Ok(())
}

fn copy_if_changed(source: &Path, target: &Path, mode: u32) -> Result<()> {
    reject_symlinks(source)?;
    reject_symlinks(target)?;
    let bytes = fs::read(source)?;
    write_if_changed(target, &bytes, mode)
}

fn write_if_changed(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    reject_symlinks(path)?;
    if path.is_file()
        && fs::read(path)? == contents
        && fs::metadata(path)?.permissions().mode() & 0o777 == mode
    {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::Invalid("staged file has no parent directory".into()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged
        .as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    staged.write_all(contents)?;
    staged.as_file().sync_all()?;
    staged
        .persist(path)
        .map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn hash_path(path: &Path) -> Result<String> {
    reject_symlinks(path)?;
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn find_file<'a>(root: &'a mut Value, path: &str) -> Result<&'a mut Mapping> {
    let entries = sequence_at_mut(root, &["storage", "files"])?;
    entries
        .iter_mut()
        .filter_map(Value::as_mapping_mut)
        .find(|entry| string_at(entry, "path").as_deref() == Some(path))
        .ok_or_else(|| Error::Invalid(format!("required Butane file entry is absent: {path}")))
}

fn set_inline(file: &mut Mapping, inline: &str) -> Result<()> {
    let contents = mapping_at_mut(file, "contents")?;
    contents.insert(Value::String("inline".into()), Value::String(inline.into()));
    Ok(())
}

fn remove_file(root: &mut Value, path: &str) -> Result<()> {
    let files = sequence_at_mut(root, &["storage", "files"])?;
    files.retain(|value| {
        value
            .as_mapping()
            .and_then(|entry| string_at(entry, "path"))
            .is_none_or(|candidate| candidate != path)
    });
    Ok(())
}

fn remove_unit(root: &mut Value, name: &str) -> Result<()> {
    let units = sequence_at_mut(root, &["systemd", "units"])?;
    units.retain(|value| {
        value
            .as_mapping()
            .and_then(|entry| string_at(entry, "name"))
            .is_none_or(|candidate| candidate != name)
    });
    Ok(())
}

fn add_authorized_key(root: &mut Value, key: &str) -> Result<()> {
    let users = sequence_at_mut(root, &["passwd", "users"])?;
    let user = users
        .iter_mut()
        .filter_map(Value::as_mapping_mut)
        .find(|user| string_at(user, "name").as_deref() == Some("giacomo"))
        .ok_or_else(|| Error::Invalid("Butane config lacks the giacomo account".into()))?;
    let keys = user
        .entry(Value::String("ssh_authorized_keys".into()))
        .or_insert_with(|| Value::Sequence(Vec::new()))
        .as_sequence_mut()
        .ok_or_else(|| Error::Invalid("SSH authorized key list is not a sequence".into()))?;
    let key = Value::String(key.trim().to_owned());
    if key.as_str().is_none_or(str::is_empty) {
        return Err(Error::Invalid("generated SSH public key is empty".into()));
    }
    if !keys.contains(&key) {
        keys.push(key);
    }
    Ok(())
}

fn add_vm_sudoers(root: &mut Value) -> Result<()> {
    let files = sequence_at_mut(root, &["storage", "files"])?;
    if files.iter().any(|value| {
        value
            .as_mapping()
            .and_then(|entry| string_at(entry, "path"))
            .as_deref()
            == Some("/etc/sudoers.d/90-skillet-vm")
    }) {
        return Ok(());
    }
    files.push(serde_yml::from_str(
        "path: /etc/sudoers.d/90-skillet-vm\nmode: 288\ncontents:\n  inline: |\n    giacomo ALL=(ALL) NOPASSWD: ALL\n",
    )
    .map_err(|error| Error::Preparation(error.to_string()))?);
    Ok(())
}

fn sequence_at_mut<'a>(root: &'a mut Value, path: &[&str]) -> Result<&'a mut Vec<Value>> {
    let mut current = root;
    for segment in path {
        current = current
            .as_mapping_mut()
            .and_then(|mapping| mapping.get_mut(Value::String((*segment).into())))
            .ok_or_else(|| {
                Error::Invalid(format!("required Butane mapping is absent: {segment}"))
            })?;
    }
    current
        .as_sequence_mut()
        .ok_or_else(|| Error::Invalid("required Butane field is not a sequence".into()))
}

fn mapping_at_mut<'a>(root: &'a mut Mapping, key: &str) -> Result<&'a mut Mapping> {
    root.get_mut(Value::String(key.into()))
        .and_then(Value::as_mapping_mut)
        .ok_or_else(|| Error::Invalid(format!("required Butane mapping is absent: {key}")))
}

fn string_at(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(Value::String(key.into()))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
