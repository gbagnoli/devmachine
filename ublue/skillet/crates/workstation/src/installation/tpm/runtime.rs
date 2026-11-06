//! Bounded native adapter. Plaintext exists only in memory and /run tmpfs.
use super::{
    enrollment::{self, Observation, Resource, State},
    invalid, predict_policy, Result,
};
use crate::installation::Error;
use serde_json::Value;
use std::{
    fs,
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};
use zeroize::Zeroizing;

const DEVICE: &str = "/dev/disk/by-partlabel/root";
const JOURNAL: &str = "/var/lib/skillet/tpm-bootstrap";

/// Called only by the explicit physical TPM install unit, before each rebase.
pub fn bootstrap(certificate: &Path) -> Result<bool> {
    if skillet_vm::current_uid() != 0 {
        return Err(invalid("run as root on the installed system"));
    }
    let directory = Path::new(JOURNAL);
    super::super::reject_symlinks(directory)?;
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join("lock"))?;
    lock.lock()?;
    let mut runtime = Native {
        certificate: fs::read(certificate)?,
        journal: directory.join("state.json"),
        device: DEVICE.into(),
    };
    enrollment::enroll(&mut runtime)
}

struct Native {
    certificate: Vec<u8>,
    journal: PathBuf,
    device: String,
}

pub(super) fn tool(program: &str, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
    let mut command = Command::new(format!("/usr/bin/{program}"));
    command
        .args(args)
        .env("LC_ALL", "C")
        .env("TMPDIR", "/run")
        .env("TPM2TOOLS_AUTOFLUSH", "true");
    let timeout = Duration::from_secs(180);
    let output = if let Some(input) = input {
        skillet_vm::capture_command_with_input(command, timeout, input)?
    } else {
        skillet_vm::capture_command(command, timeout)?
    };
    if !output.status.success() {
        // Tool stderr can be sensitive: retain it in root-only tmpfs, never in
        // the system journal. Stdout may be a key and is never retained.
        let mut diagnostic = tempfile::Builder::new()
            .prefix("skillet-tpm-failure-")
            .suffix(".log")
            .tempfile_in("/run")?;
        diagnostic.write_all(&output.stderr)?;
        let (_, path) = diagnostic.keep().map_err(|e| e.error)?;
        return Err(invalid(&format!(
            "{program} {} failed (exit {:?}); private stderr: {}",
            args.first().copied().unwrap_or(""),
            output.status.code(),
            path.display()
        )));
    }
    Ok(output)
}

fn metadata(device: &str) -> Result<Value> {
    let output = tool(
        "cryptsetup",
        &["luksDump", "--dump-json-metadata", device],
        None,
    )?;
    serde_json::from_slice(&output.stdout).map_err(|_| invalid("invalid LUKS2 metadata JSON"))
}

fn volume(device: &str) -> Result<String> {
    let bytes = tool("cryptsetup", &["luksUUID", device], None)?.stdout;
    let uuid = String::from_utf8(bytes).map_err(|_| invalid("invalid root UUID"))?;
    let uuid = uuid.trim();
    if uuid.len() != 36 || !uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err(invalid("invalid root UUID"));
    }
    Ok(uuid.to_owned())
}

fn check_volume(device: &str, state: &State) -> Result<()> {
    if volume(device)? != state.volume {
        return Err(invalid("root volume changed during enrollment"));
    }
    Ok(())
}

fn unseal(jwe: &str) -> Result<Zeroizing<Vec<u8>>> {
    let key = Zeroizing::new(tool("clevis", &["decrypt"], Some(jwe.as_bytes()))?.stdout);
    if key.is_empty() || key.len() > 4096 {
        return Err(invalid("unexpected unsealed key size"));
    }
    Ok(key)
}

pub(super) fn keyfile(key: &[u8]) -> Result<tempfile::NamedTempFile> {
    let output = tool(
        "findmnt",
        &["--noheadings", "--output", "FSTYPE", "--target", "/run"],
        None,
    )?;
    if output.stdout != b"tmpfs\n" {
        return Err(invalid("/run must be tmpfs for transient enrollment keys"));
    }
    let mut file = tempfile::Builder::new()
        .prefix("skillet-tpm-key-")
        .tempfile_in("/run")?;
    file.write_all(key)?;
    Ok(file)
}

fn test_slot(device: &str, file: &Path, slot: u8) -> Result<()> {
    let file = file
        .to_str()
        .ok_or_else(|| invalid("invalid tmpfs key path"))?;
    tool(
        "cryptsetup",
        &[
            "open",
            "--test-passphrase",
            "--key-slot",
            &slot.to_string(),
            "--key-file",
            file,
            device,
        ],
        None,
    )?;
    Ok(())
}

fn read_pcr7() -> Result<[u8; 32]> {
    let pcr_file = tempfile::Builder::new()
        .prefix("skillet-pcr7-")
        .tempfile_in("/run")?;
    let path = pcr_file
        .path()
        .to_str()
        .ok_or_else(|| invalid("invalid PCR path"))?;
    tool("tpm2_pcrread", &["-Q", "sha256:7", "-o", path], None)?;
    fs::read(pcr_file.path())?
        .try_into()
        .map_err(|_| invalid("TPM returned an unexpected SHA256 PCR size"))
}

impl Resource for Native {
    fn observe(&mut self) -> Result<Observation> {
        // Refuse installation-time mutations against an unrelated labelled disk.
        if !Path::new("/run/ostree-booted").exists() {
            return Err(invalid(
                "transition enrollment requires an installed ostree system",
            ));
        }
        tool(
            "findmnt",
            &[
                "--noheadings",
                "--output",
                "TARGET",
                "--source",
                "/dev/mapper/root",
                "--target",
                "/sysroot",
            ],
            None,
        )?;
        let active = tool("cryptsetup", &["status", "root"], None)?;
        let status =
            String::from_utf8(active.stdout).map_err(|_| invalid("invalid root mapper status"))?;
        let backing = status
            .lines()
            .find_map(|line| line.trim().strip_prefix("device:").map(str::trim))
            .ok_or_else(|| invalid("root mapper has no backing device"))?;
        if fs::canonicalize(backing)? != fs::canonicalize(&self.device)? {
            return Err(invalid("root label does not match the active root mapper"));
        }
        let observed = read_pcr7()?;
        let log = fs::read("/sys/kernel/security/tpm0/binary_bios_measurements")?;
        Ok(Observation {
            volume: volume(&self.device)?,
            policy: predict_policy(&log, &self.certificate, observed)?,
            metadata: metadata(&self.device)?,
        })
    }

    fn load(&mut self) -> Result<Option<State>> {
        match fs::read(&self.journal) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| invalid("invalid retained bootstrap journal")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&mut self, state: &State) -> Result<()> {
        check_volume(&self.device, state)?;
        let parent = self
            .journal
            .parent()
            .ok_or_else(|| invalid("journal has no parent"))?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(&mut file, state)
            .map_err(|_| invalid("cannot serialize bootstrap journal"))?;
        file.as_file().sync_all()?;
        file.persist(&self.journal)
            .map_err(|e| Error::Io(e.error))?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    fn seal(&mut self, source: &str, target: [u8; 32]) -> Result<String> {
        let key = unseal(source)?;
        super::sealing::seal(&key, target)
    }

    fn install(&mut self, state: &State) -> Result<()> {
        check_volume(&self.device, state)?;
        let snapshot = metadata(&self.device)?;
        let target = state.target_slot.to_string();
        let token_id = state.target_token.to_string();
        let expected = enrollment::token(state)?;
        if let Some(present) = snapshot["tokens"].get(&token_id) {
            if present != &expected {
                return Err(invalid("reserved token is occupied by another binding"));
            }
        }
        // Either approved state can recover an interrupted add/import sequence.
        let current = read_pcr7()?;
        let jwe = if current == state.policy.target {
            &state.target_jwe
        } else if current == state.policy.initial {
            &state.source_jwe
        } else {
            return Err(invalid("boot measurements changed during enrollment"));
        };
        let key = unseal(jwe)?;
        let file = keyfile(&key)?;
        test_slot(&self.device, file.path(), state.source_slot)?;
        if snapshot["keyslots"].get(&target).is_none() {
            let path = file
                .path()
                .to_str()
                .ok_or_else(|| invalid("invalid key path"))?;
            check_volume(&self.device, state)?;
            tool(
                "cryptsetup",
                &[
                    "luksAddKey",
                    "--batch-mode",
                    "--key-slot",
                    &target,
                    "--key-file",
                    path,
                    &self.device,
                    path,
                ],
                None,
            )?;
        }
        // A reserved slot left by interruption must accept the journal's key.
        test_slot(&self.device, file.path(), state.target_slot)?;
        if snapshot["tokens"].get(&token_id).is_none() {
            let bytes = serde_json::to_vec(&expected).map_err(|_| invalid("invalid token JSON"))?;
            check_volume(&self.device, state)?;
            tool(
                "cryptsetup",
                &["token", "import", "--token-id", &token_id, &self.device],
                Some(&bytes),
            )?;
        }
        if metadata(&self.device)?["tokens"].get(&token_id) != Some(&expected) {
            return Err(invalid(
                "installed token did not retain the intended sealed key",
            ));
        }
        check_volume(&self.device, state)
    }

    fn verify(&mut self, state: &State, target_boot: bool) -> Result<()> {
        check_volume(&self.device, state)?;
        let key = unseal(if target_boot {
            &state.target_jwe
        } else {
            &state.source_jwe
        })?;
        let file = keyfile(&key)?;
        test_slot(&self.device, file.path(), state.source_slot)?;
        test_slot(&self.device, file.path(), state.target_slot)?;
        check_volume(&self.device, state)
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
