use super::*;
use crate::installation::tpm::{enrollment::compact_jwe, extend, validate_jwe, Policy};
use sha2::{Digest as _, Sha256};

/// Run only in the disposable container driver. It owns a software TPM and a
/// regular sparse file; no host block device, device mapper or vault is used.
#[test]
#[ignore = "requires the isolated tpm-bootstrap.sh container and software TPM"]
fn software_tpm_transition_and_interrupted_token_import() {
    assert_eq!(
        std::env::var("SKILLET_SOFTWARE_TPM_ACCEPTANCE").as_deref(),
        Ok("1")
    );
    let tcti = std::env::var("TPM2TOOLS_TCTI").unwrap();
    assert_eq!(tcti, "swtpm:host=127.0.0.1,port=2321");
    assert!(Path::new("/run/.containerenv").is_file());
    let work = tempfile::tempdir_in("/run").unwrap();
    let device = work.path().join("disposable.luks");
    fs::File::create(&device)
        .unwrap()
        .set_len(128 * 1024 * 1024)
        .unwrap();
    let device = device.to_str().unwrap();
    create_initial_binding(device);
    check_transition(work.path(), device);
}

fn create_initial_binding(device: &str) {
    let mut original = tempfile::NamedTempFile::new_in("/run").unwrap();
    original.write_all(b"disposable fixture password").unwrap();
    let path = original.path().to_str().unwrap();
    tool(
        "cryptsetup",
        &[
            "luksFormat",
            "--batch-mode",
            "--type",
            "luks2",
            "--pbkdf",
            "pbkdf2",
            "--pbkdf-force-iterations",
            "1000",
            "--key-file",
            path,
            device,
        ],
        None,
    )
    .unwrap();
    tool(
        "clevis",
        &[
            "luks",
            "bind",
            "-y",
            "-d",
            device,
            "-s",
            "1",
            "-k",
            path,
            "tpm2",
            "{\"hash\":\"sha256\",\"pcr_bank\":\"sha256\",\"pcr_ids\":\"7\"}",
        ],
        None,
    )
    .unwrap();
    let source = compact_jwe(&metadata(device).unwrap()["tokens"]["0"]["jwe"]).unwrap();
    let bound_key = unseal(&source).unwrap();
    let bound_file = keyfile(&bound_key).unwrap();
    tool(
        "cryptsetup",
        &[
            "luksKillSlot",
            "--batch-mode",
            "--key-file",
            bound_file.path().to_str().unwrap(),
            device,
            "0",
        ],
        None,
    )
    .unwrap();
}

fn check_transition(work: &Path, device: &str) {
    let initial = read_pcr7().unwrap();
    let event: [u8; 32] = Sha256::digest(b"fixture additional approved authority").into();
    let target = extend(initial, event);
    let first = metadata(device).unwrap();
    let source = compact_jwe(&first["tokens"]["0"]["jwe"]).unwrap();
    validate_jwe(&source, initial).unwrap();
    let mut native = Native {
        certificate: Vec::new(),
        journal: work.join("state.json"),
        device: device.into(),
    };
    let sealed = native.seal(&source, target).unwrap();
    validate_jwe(&sealed, target).unwrap();
    let state = State {
        version: 1,
        volume: volume(device).unwrap(),
        policy: Policy {
            initial,
            target,
            certificate_sha256: "fixture".into(),
        },
        source_slot: 1,
        target_slot: 2,
        target_token: 1,
        source_jwe: source.clone(),
        target_jwe: sealed.clone(),
        verified_target_boot: false,
    };
    native.save(&state).unwrap();
    // Interrupt after keyslot creation but before token import, then retry.
    let key = unseal(&source).unwrap();
    let file = keyfile(&key).unwrap();
    let path = file.path().to_str().unwrap();
    tool(
        "cryptsetup",
        &[
            "luksAddKey",
            "--batch-mode",
            "--key-slot",
            "2",
            "--key-file",
            path,
            device,
            path,
        ],
        None,
    )
    .unwrap();
    drop(file);
    drop(key);
    assert!(metadata(device).unwrap()["tokens"].get("1").is_none());
    let retained = native.load().unwrap().unwrap();
    native.install(&retained).unwrap();
    native.verify(&retained, false).unwrap();
    let prepared = metadata(device).unwrap();
    let mut conflict = retained.clone();
    conflict.target_token = 0;
    assert!(native.install(&conflict).is_err());
    assert_eq!(metadata(device).unwrap(), prepared);
    native.install(&retained).unwrap();
    assert_eq!(metadata(device).unwrap(), prepared);
    assert_eq!(prepared["tokens"]["0"], first["tokens"]["0"]);
    assert!(
        unseal(&sealed).is_err(),
        "future policy must not unlock early"
    );
    tool(
        "tpm2_pcrextend",
        &[&format!("7:sha256={}", hex::encode(event))],
        None,
    )
    .unwrap();
    assert_eq!(read_pcr7().unwrap(), target);
    assert!(
        unseal(&source).is_err(),
        "old policy must fail after transition"
    );
    native.verify(&retained, true).unwrap();
    native.install(&retained).unwrap();
    assert_eq!(metadata(device).unwrap(), prepared);
    verify_recovery_after_transition(device);
    // An additional, unapproved measurement must deny both paths.
    tool(
        "tpm2_pcrextend",
        &[&format!("7:sha256={}", hex::encode([1; 32]))],
        None,
    )
    .unwrap();
    assert!(unseal(&source).is_err());
    assert!(unseal(&sealed).is_err());
    println!("PASS: initial/future restricted unlock, interrupted import retry, repeat enrollment, and unapproved-PCR denial");
}

fn verify_recovery_after_transition(device: &str) {
    // The old slot sorts first and cannot unseal. Recovery must try the working
    // target binding rather than assuming the first listed slot always works.
    let directory = Path::new("/dev/disk/by-partlabel");
    fs::create_dir_all(directory).unwrap();
    std::os::unix::fs::symlink(device, directory.join("root")).unwrap();
    let recovery = "ab".repeat(64);
    tool(
        "bash",
        &["/root-recovery.sh", "enroll"],
        Some(recovery.as_bytes()),
    )
    .unwrap();
    let file = keyfile(recovery.as_bytes()).unwrap();
    test_slot(device, file.path(), 0).unwrap();
    assert_eq!(
        metadata(device).unwrap()["keyslots"]
            .as_object()
            .unwrap()
            .len(),
        3
    );
}
