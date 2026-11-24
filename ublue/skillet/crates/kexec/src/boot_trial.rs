//! Firmware boot trials use a persistent GRUB attempt budget.
use super::{parse_entry, runtime, Error};
use serde_json::Value;
use std::{fs, io, path::Path, process::Command};

const HOOK: &str = "/usr/lib/skillet/boot-trial/grub.cfg";
const BEFORE: &str = "/boot/loader/skillet-trial-before-finalize";
const CUSTOM: &str = "/boot/grub2/custom.cfg";
pub(crate) const ENV: &str = "/boot/grub2/skillet-trial.env";
pub(crate) const ENTRY: &str = "/boot/loader/entries/skillet-trial.conf";
const MARKER: &str = "skillet.boot-trial=1";
pub(crate) const OPT_IN: &str = "/etc/skillet/boot-trials";
const SUCCESS: &str = "/var/lib/skillet/boot-trial-success";
pub(crate) const QUARANTINE: &str = "/var/lib/skillet/boot-trial-quarantine";

fn run(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

pub(crate) fn status() -> Result<Value, runtime::RuntimeError> {
    Ok(serde_json::from_str(&run(
        "rpm-ostree",
        &["status", "--json"],
    )?)?)
}

fn digest(item: &Value) -> io::Result<&str> {
    item["base-commit-meta"]["ostree.manifest-digest"]
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| io::Error::other("deployment lacks a verified image digest"))
}

fn entry(item: &Value) -> Result<(String, String), runtime::RuntimeError> {
    let checksum = item["checksum"]
        .as_str()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(Error::InvalidDeployment)?;
    let os = item["osname"]
        .as_str()
        .filter(|value| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
        .ok_or(Error::InvalidDeployment)?;
    let serial = item["serial"].as_u64().ok_or(Error::InvalidDeployment)?;
    let expected = fs::canonicalize(format!(
        "/sysroot/ostree/deploy/{os}/deploy/{checksum}.{serial}"
    ))?;
    let mut selected = None;
    for file in fs::read_dir("/boot/loader/entries")? {
        let file = file?;
        let name = file.file_name();
        let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".conf")) else {
            continue;
        };
        if !id.starts_with("ostree-") || !file.file_type()?.is_file() {
            continue;
        }
        let content = fs::read_to_string(file.path())?;
        let Ok(parsed) = parse_entry(&content, Path::new("/boot")) else {
            continue;
        };
        let actual = fs::canonicalize(
            Path::new("/sysroot").join(
                parsed
                    .deployment_selector
                    .strip_prefix("/")
                    .map_err(io::Error::other)?,
            ),
        );
        if actual.is_ok_and(|path| path == expected)
            && selected.replace((id.to_owned(), content)).is_some()
        {
            return Err(Error::InvalidDeployment.into());
        }
    }
    selected.ok_or_else(|| Error::InvalidDeployment.into())
}

fn booted(status: &Value) -> Result<&Value, Error> {
    let mut items = status["deployments"]
        .as_array()
        .ok_or(Error::InvalidDeployment)?
        .iter()
        .filter(|item| item["booted"] == true);
    let item = items.next().ok_or(Error::InvalidDeployment)?;
    if items.next().is_some() {
        return Err(Error::InvalidDeployment);
    }
    Ok(item)
}

fn atomic(path: &str, contents: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let parent = Path::new(path)
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    fs::File::open(parent)?.sync_all()
}

fn arm_environment(fallback: &str, trial: &str, previous: &str) -> io::Result<()> {
    let temp = tempfile::NamedTempFile::new_in("/boot/grub2")?;
    let path = temp
        .path()
        .to_str()
        .ok_or_else(|| io::Error::other("invalid GRUB environment path"))?;
    run("grub2-editenv", &[path, "create"])?;
    run(
        "grub2-editenv",
        &[
            path,
            "set",
            "skillet_trial_candidate=skillet-trial",
            fallback,
            "skillet_trial_attempts=2",
            trial,
            previous,
        ],
    )?;
    atomic(ENV, &fs::read(path)?)
}

pub(crate) fn preflight(target: &Value) -> Result<(), runtime::RuntimeError> {
    digest(target)?;
    if Path::new(CUSTOM).exists()
        || Path::new(ENV).exists()
        || Path::new(ENTRY).exists()
        || Path::new(QUARANTINE).exists()
    {
        return Err(io::Error::other(
            "unresolved boot trial, quarantine, or existing GRUB custom configuration",
        )
        .into());
    }
    if !fs::read_to_string("/boot/grub2/grub.cfg")?.contains("source $prefix/custom.cfg")
        || run("findmnt", &["-n", "-o", "FSTYPE", "/boot"])?.trim() != "ext4"
        || !Path::new(HOOK).is_file()
    {
        return Err(io::Error::other(
            "boot trials require the bootupd GRUB hook, ext4 /boot and the image trial module",
        )
        .into());
    }
    Ok(())
}

/// Protect the current boot on both sides of the native BLS directory swap.
pub(crate) fn guard(previous: &Value, target: &Value) -> Result<(), runtime::RuntimeError> {
    entry(previous)?;
    if target["staged"] != true
        || Path::new(BEFORE).try_exists()?
        || !matches!(
            fs::read_link("/boot/loader")?.to_str(),
            Some("loader.0" | "loader.1")
        )
    {
        return Err(io::Error::other(
            "boot trial requires a staged update and the booted default OSTree entry",
        )
        .into());
    }
    let trial = format!("skillet_trial_digest={}", digest(target)?);
    let prior = format!("skillet_trial_previous={}", digest(previous)?);
    let hook = fs::read(HOOK)?;
    run("mount", &["-o", "remount,rw", "/boot"])?;
    let result = (|| -> io::Result<()> {
        atomic(BEFORE, b"known-good loader directory\n")?;
        run("grub2-editenv", &[ENV, "create"])?;
        run(
            "grub2-editenv",
            &[
                ENV,
                "set",
                "skillet_trial_fallback=1",
                "skillet_trial_attempts=0",
                &trial,
                &prior,
            ],
        )?;
        fs::File::open(ENV)?.sync_all()?;
        atomic(CUSTOM, &hook)?;
        Ok(())
    })();
    let remount = run("mount", &["-o", "remount,ro", "/boot"]);
    result?;
    remount?;
    Ok(())
}

pub(crate) fn arm(previous: &Value, target: &Value) -> Result<(), runtime::RuntimeError> {
    let (fallback, _) = entry(previous)?;
    let (_, content) = entry(target)?;
    if Path::new(BEFORE).try_exists()? {
        return Err(io::Error::other("OSTree did not publish the expected fallback layout").into());
    }
    let parsed = parse_entry(&content, Path::new("/boot"))?;
    let initrd = parsed.initrd.to_str().ok_or(Error::InvalidPath)?;
    if !run("lsinitrd", &["-m", initrd])?
        .lines()
        .any(|line| line.trim() == "skillet-boot-trial")
    {
        return Err(io::Error::other("candidate initramfs lacks the boot trial deadline").into());
    }
    let content = content
        .lines()
        .map(|line| {
            if line.starts_with("options ") {
                format!("{line} {MARKER}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    let trial_digest = format!("skillet_trial_digest={}", digest(target)?);
    let fallback_digest = format!("skillet_trial_previous={}", digest(previous)?);
    let fallback = format!("skillet_trial_fallback={fallback}");
    run("mount", &["-o", "remount,rw", "/boot"])?;
    let result = (|| -> io::Result<()> {
        atomic(ENTRY, content.as_bytes())?;
        arm_environment(&fallback, &trial_digest, &fallback_digest)?;
        fs::File::open(ENV)?.sync_all()?;
        atomic(CUSTOM, &fs::read(HOOK)?)?;
        run("sync", &["-f", "/boot"])?;
        Ok(())
    })();
    let remount = run("mount", &["-o", "remount,ro", "/boot"]);
    result?;
    remount?;
    Ok(())
}

pub(crate) fn conclude() -> Result<(), runtime::RuntimeError> {
    if !Path::new(ENV).exists() {
        return Ok(());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    let _lock = loop {
        match skillet_core::maintenance::activation() {
            Ok(lock) => break lock,
            Err(error) if std::time::Instant::now() >= deadline => return Err(error.into()),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(250)),
        }
    };
    let values = run("grub2-editenv", &[ENV, "list"])?;
    let get = |key: &str| {
        values.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key).then_some(value)
        })
    };
    let current = status()?;
    if current
        .get("transaction")
        .is_none_or(|transaction| !transaction.is_null())
    {
        return Err(Error::InvalidDeployment.into());
    }
    let active = digest(booted(&current)?)?;
    let trial = get("skillet_trial_digest")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(Error::InvalidDeployment)?;
    match fs::read(CUSTOM) {
        Ok(contents) if contents == fs::read(HOOK)? => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        _ => {
            return Err(
                io::Error::other("GRUB custom configuration changed during boot trial").into(),
            )
        }
    }
    let is_trial = fs::read_to_string("/proc/cmdline")?
        .split_ascii_whitespace()
        .any(|arg| arg == MARKER);
    if is_trial && active == trial {
        run("mountpoint", &["-q", "/var/lib/data"])?;
        run("systemctl", &["is-active", "sshd.service"])?;
        if current["deployments"][0]["booted"] != true {
            return Err(Error::InvalidDeployment.into());
        }
        atomic(SUCCESS, format!("sha256:{trial}\n").as_bytes())?;
    } else if !is_trial
        && get("skillet_trial_attempts") == Some("0")
        && get("skillet_trial_previous") == Some(active)
    {
        let default = &current["deployments"][0];
        let remove_candidate = default["booted"] != true;
        if remove_candidate && digest(default)? != trial {
            return Err(Error::InvalidDeployment.into());
        }
        atomic(QUARANTINE, format!("sha256:{trial}\n").as_bytes())?;
        if remove_candidate {
            // Drop the rejected default while retaining the booted deployment.
            // A swap could repoint the existing GRUB fallback ID at the bad image.
            run("rpm-ostree", &["cleanup", "--pending"])?;
            if status()?["deployments"][0]["booted"] != true {
                return Err(Error::InvalidDeployment.into());
            }
        }
        tracing::error!(
            "OS trial exhausted its boot attempts; staging suspended for sha256:{trial}"
        );
    } else {
        return Err(Error::InvalidDeployment.into());
    }
    run("mount", &["-o", "remount,rw", "/boot"])?;
    let result = (|| -> io::Result<()> {
        for path in [CUSTOM, ENTRY, BEFORE] {
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        fs::File::open("/boot/loader/entries")?.sync_all()?;
        fs::File::open("/boot/grub2")?.sync_all()?;
        fs::remove_file(ENV)?;
        run("sync", &["-f", "/boot"])?;
        Ok(())
    })();
    let remount = run("mount", &["-o", "remount,ro", "/boot"]);
    result?;
    remount?;
    Ok(())
}
