use super::*;
use enrollment::{Observation, Resource, State};
use serde_json::{json, Value};

fn certificate() -> &'static [u8] {
    include_bytes!("fixtures/test-cert.der")
}

fn variable(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![
        0x61, 0xdf, 0xe4, 0x8b, 0xca, 0x93, 0xd2, 0x11, 0xaa, 0x0d, 0x00, 0xe0, 0x98, 0x03, 0x2b,
        0x8c,
    ];
    bytes.extend(u64::try_from(name.len()).unwrap().to_le_bytes());
    bytes.extend(u64::try_from(payload.len()).unwrap().to_le_bytes());
    for character in name.encode_utf16() {
        bytes.extend(character.to_le_bytes());
    }
    bytes.extend(payload);
    bytes
}

fn log(authority: bool, enabled: bool) -> (Vec<u8>, [u8; 32]) {
    let mut spec = b"Spec ID Event03\0".to_vec();
    spec.extend([0, 0, 0, 0, 0, 2, 0, 2]);
    spec.extend(1_u32.to_le_bytes());
    spec.extend(11_u16.to_le_bytes());
    spec.extend(32_u16.to_le_bytes());
    spec.push(0);
    let mut log = vec![0; 4];
    log.extend(3_u32.to_le_bytes());
    log.extend([0; 20]);
    log.extend(u32::try_from(spec.len()).unwrap().to_le_bytes());
    log.extend(spec);
    let mut pcr = [0; 32];
    let mut events = vec![(
        0x8000_0001_u32,
        variable("SecureBoot", &[u8::from(enabled)]),
    )];
    if authority {
        events.push((0x8000_00e0, authority_event(certificate()).unwrap()));
    }
    for (kind, data) in events {
        let digest: [u8; 32] = Sha256::digest(&data).into();
        log.extend(7_u32.to_le_bytes());
        log.extend(kind.to_le_bytes());
        log.extend(1_u32.to_le_bytes());
        log.extend(11_u16.to_le_bytes());
        log.extend(digest);
        log.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
        log.extend(data);
        pcr = extend(pcr, digest);
    }
    (log, pcr)
}

#[test]
fn replays_current_and_predicts_exact_extra_authority_without_reextending_it() {
    let (initial_log, initial) = log(false, true);
    let (target_log, target) = log(true, true);
    let predicted = predict_policy(&initial_log, certificate(), initial).unwrap();
    assert_eq!(predicted.target, target);
    let booted = predict_policy(&target_log, certificate(), target).unwrap();
    assert_eq!(booted.initial, booted.target);
    assert!(predict_policy(&initial_log, certificate(), target).is_err());
    assert!(predict_policy(&initial_log, b"not a certificate", initial).is_err());
    let (disabled, pcr) = log(false, false);
    assert!(predict_policy(&disabled, certificate(), pcr).is_err());
}

#[test]
fn rejects_truncated_tampered_and_unsupported_logs_without_panicking() {
    let (valid, pcr) = log(false, true);
    for end in 0..valid.len() {
        assert!(predict_policy(&valid[..end], certificate(), pcr).is_err());
    }
    let mut tampered = valid.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(predict_policy(&tampered, certificate(), pcr).is_err());
    let mut unsupported = valid;
    unsupported[32] ^= 1;
    assert!(predict_policy(&unsupported, certificate(), pcr).is_err());
}

fn jwe(pcr: [u8; 32]) -> String {
    let mut public = vec![0, 46, 0, 8, 0, 11, 0, 0, 0, 0x92, 0, 32];
    public.extend(policy_digest(pcr));
    public.extend([0, 0, 0, 0]);
    let header = json!({"clevis":{"pin":"tpm2", "tpm2":{
        "hash":"sha256", "pcr_bank":"sha256", "pcr_ids":"7",
        "jwk_pub":URL_SAFE_NO_PAD.encode(public)
    }}});
    format!(
        "{}..iv.ciphertext.tag",
        URL_SAFE_NO_PAD.encode(header.to_string())
    )
}

#[test]
fn sealed_objects_must_bind_the_exact_policy_and_forbid_password_bypass() {
    let valid = jwe([1; 32]);
    validate_jwe(&valid, [1; 32]).unwrap();
    assert!(validate_jwe(&valid, [2; 32]).is_err());
    let mut value = header(&valid).unwrap();
    value["clevis"]["tpm2"]["pcr_ids"] = json!("0");
    let altered = format!(
        "{}..iv.ciphertext.tag",
        URL_SAFE_NO_PAD.encode(value.to_string())
    );
    assert!(validate_jwe(&altered, [1; 32]).is_err());
    let mut value = header(&valid).unwrap();
    let mut public = URL_SAFE_NO_PAD
        .decode(value["clevis"]["tpm2"]["jwk_pub"].as_str().unwrap())
        .unwrap();
    public[9] |= 0x40;
    value["clevis"]["tpm2"]["jwk_pub"] = json!(URL_SAFE_NO_PAD.encode(public));
    let altered = format!(
        "{}..iv.ciphertext.tag",
        URL_SAFE_NO_PAD.encode(value.to_string())
    );
    assert!(validate_jwe(&altered, [1; 32]).is_err());
}

struct Fake {
    state: Option<State>,
    policy: Policy,
    volume: String,
    metadata: Value,
    allocations: usize,
    fail_install: bool,
    verified: Vec<bool>,
}
impl Fake {
    fn fresh() -> Self {
        let (log, pcr) = log(false, true);
        let policy = predict_policy(&log, certificate(), pcr).unwrap();
        let source = jwe(policy.initial);
        Self {
            metadata: json!({"keyslots":{"1":{}}, "tokens":{"0":{
                "type":"clevis", "keyslots":["1"], "jwe":source
            }}}),
            state: None,
            policy,
            volume: "fixture-volume".into(),
            allocations: 0,
            fail_install: false,
            verified: Vec::new(),
        }
    }
}
impl Resource for Fake {
    fn observe(&mut self) -> Result<Observation> {
        Ok(Observation {
            volume: self.volume.clone(),
            policy: self.policy.clone(),
            metadata: self.metadata.clone(),
        })
    }
    fn load(&mut self) -> Result<Option<State>> {
        Ok(self.state.clone())
    }
    fn save(&mut self, state: &State) -> Result<()> {
        self.state = Some(state.clone());
        Ok(())
    }
    fn seal(&mut self, _: &str, target: [u8; 32]) -> Result<String> {
        self.allocations += 1;
        Ok(jwe(target))
    }
    fn install(&mut self, state: &State) -> Result<()> {
        assert!(
            self.state.is_some(),
            "must persist sealed keys before LUKS mutation"
        );
        if self.fail_install {
            return Err(invalid("injected interrupted installation"));
        }
        self.metadata["keyslots"][state.target_slot.to_string()] = json!({});
        self.metadata["tokens"][state.target_token.to_string()] = enrollment::token(state)?;
        Ok(())
    }
    fn verify(&mut self, _: &State, target: bool) -> Result<()> {
        self.verified.push(target);
        Ok(())
    }
}

#[test]
fn enrollment_retries_and_verifies_target_boot_without_rotating_or_deleting_source() {
    let mut resource = Fake::fresh();
    let original = resource.metadata["tokens"]["0"].clone();
    resource.fail_install = true;
    assert!(enrollment::enroll(&mut resource).is_err());
    assert!(resource.state.is_some());
    resource.fail_install = false;
    assert!(!enrollment::enroll(&mut resource).unwrap());
    assert!(!enrollment::enroll(&mut resource).unwrap());
    assert_eq!(resource.allocations, 1);
    resource.policy.initial = resource.policy.target;
    assert!(enrollment::enroll(&mut resource).unwrap());
    assert!(enrollment::enroll(&mut resource).unwrap());
    assert_eq!(resource.metadata["tokens"]["0"], original);
    assert_eq!(resource.verified, [false, false, true]);
    assert_eq!(resource.allocations, 1);
}

#[test]
fn journal_rejects_another_disk_certificate_or_unapproved_boot_before_mutation() {
    for mutation in 0..3 {
        let mut resource = Fake::fresh();
        enrollment::enroll(&mut resource).unwrap();
        match mutation {
            0 => resource.volume = "different-volume".into(),
            1 => resource.policy.certificate_sha256 = "different-certificate".into(),
            _ => resource.policy.initial = [0; 32],
        }
        assert!(enrollment::enroll(&mut resource).is_err());
        assert_eq!(resource.verified.len(), 1);
    }
}

#[test]
fn already_measured_authority_is_verified_without_mutation_and_tolerates_later_recovery() {
    let mut resource = Fake::fresh();
    resource.policy.initial = resource.policy.target;
    resource.metadata["tokens"]["0"]["jwe"] = json!(jwe(resource.policy.initial));
    let original = resource.metadata.clone();
    assert!(enrollment::enroll(&mut resource).unwrap());
    assert_eq!(resource.allocations, 0);
    assert_eq!(resource.metadata, original);
    resource.metadata["keyslots"]["2"] = json!({});
    assert!(enrollment::enroll(&mut resource).unwrap());
    assert_eq!(resource.allocations, 0);
}

#[test]
fn physical_media_orders_transition_before_rebase_and_keeps_plain_profile_separate() {
    let mut config: yaml_serde::Value =
        yaml_serde::from_str(include_str!("../../../../../../butane/clamps.bu")).unwrap();
    configure_bootstrap(&mut config, certificate()).unwrap();
    let rendered = yaml_serde::to_string(&config).unwrap();
    assert!(rendered.contains("Requires=skillet-tpm-bootstrap.service"));
    assert!(rendered.contains("After=skillet-tpm-bootstrap.service"));
    assert!(rendered.contains("install tpm-bootstrap --certificate"));
    assert!(rendered.contains("AssertPathExists=/etc/skillet-install/ublue.der"));
    assert!(!rendered.contains("--wait"));
    assert!(configure_bootstrap(&mut config, certificate()).is_err());
}

#[test]
#[ignore = "requires private captured boot reports and the supplied trusted public certificate"]
fn captured_reports_predict_the_observed_target() {
    let root = std::path::PathBuf::from(std::env::var_os("SKILLET_TPM_REPORTS").unwrap());
    let certificate = std::fs::read(std::env::var_os("SKILLET_TPM_CERTIFICATE").unwrap()).unwrap();
    let read = |name: &str| {
        let directory = root.join(name);
        let text = std::fs::read_to_string(directory.join("pcr7.txt")).unwrap();
        let (_, hex) = text.split_once("0x").unwrap();
        let pcr: [u8; 32] = hex::decode(hex.trim()).unwrap().try_into().unwrap();
        let log = std::fs::read(directory.join("events.bin")).unwrap();
        (log, pcr)
    };
    let (initial_log, initial) = read("tpm-previous");
    let (target_log, target) = read("tpm-failing");
    let prediction = predict_policy(&initial_log, &certificate, initial).unwrap();
    assert_eq!(prediction.target, target);
    let booted = predict_policy(&target_log, &certificate, target).unwrap();
    assert_eq!(booted.initial, booted.target);
}

#[test]
#[ignore = "requires existing Podman and disposable Fedora software-TPM tools"]
fn real_software_tpm_bootstrap_driver() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../integration_tests/tpm-bootstrap.sh");
    let mut command = std::process::Command::new("bash");
    command.arg(script).arg(std::env::current_exe().unwrap());
    crate::installation::run(command, "isolated software-TPM acceptance").unwrap();
}

#[test]
#[ignore = "requires existing Podman and the Butane compiler container"]
fn physical_tpm_payload_passes_real_butane_merge() {
    use std::{
        collections::BTreeMap, fs, os::unix::fs::PermissionsExt as _, path::Path, process::Command,
    };
    let temporary = tempfile::tempdir().unwrap();
    let workspace = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let butane = fs::canonicalize(workspace.join("../butane")).unwrap();
    let request = crate::installation::Request {
        host: "clamps",
        disk: "/dev/disk/by-id/ata-disposable",
        root_profile: skillet_vm::install::RootProfile::Tpm,
        iso: Path::new("unused.iso"),
        output: temporary.path(),
        public_key: Path::new("unused.pub"),
        password_hash: Path::new("unused.hash"),
        mok_certificate: None,
        butane: &butane,
        workspace: &workspace,
    };
    let mut config: yaml_serde::Value =
        yaml_serde::from_str(&fs::read_to_string(butane.join("clamps.bu")).unwrap()).unwrap();
    crate::installation::specialize(&mut config, request.disk, request.root_profile).unwrap();
    skillet_vm::install::normalize_butane_modes(&mut config).unwrap();
    configure_bootstrap(&mut config, certificate()).unwrap();
    let mut passwd: yaml_serde::Value =
        yaml_serde::from_str(&fs::read_to_string(butane.join("includes/passwd.bu")).unwrap())
            .unwrap();
    skillet_vm::install::normalize_butane_modes(&mut passwd).unwrap();
    let dummy = temporary.path().join("dummy-binary");
    fs::write(&dummy, b"compiler fixture, never executed").unwrap();
    fs::set_permissions(&dummy, fs::Permissions::from_mode(0o755)).unwrap();
    let binaries = BTreeMap::from([
        ("skillet".into(), dummy.clone()),
        ("skillet-clamps".into(), dummy),
    ]);
    let staged =
        crate::installation::stage_source(&request, temporary.path(), &config, &passwd, &binaries)
            .unwrap();
    let output = temporary.path().join("compiled");
    let mut compiler = Command::new(butane.join("bin/butane"));
    compiler
        .args(["-s", "-o"])
        .arg(&output)
        .arg(staged.join("clamps.bu"));
    crate::installation::run(compiler, "TPM Butane compiler acceptance").unwrap();
    let ignition: Value =
        serde_json::from_slice(&fs::read(output.join("clamps.ign")).unwrap()).unwrap();
    let units = ignition["systemd"]["units"].as_array().unwrap();
    let bootstrap = units
        .iter()
        .find(|u| u["name"] == "ucore-bootstrap.service")
        .unwrap();
    // Butane embeds merge fragments as data URLs; Ignition performs the final
    // merge on boot. Preserve the base unit and layer only the TPM dependency.
    let fragments: Vec<Value> = ignition["ignition"]["config"]["merge"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fragment| {
            let encoded = fragment["source"]
                .as_str()
                .unwrap()
                .strip_prefix("data:;base64,")
                .unwrap();
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap();
            let mut bytes = Vec::new();
            if fragment["compression"] == "gzip" {
                use std::io::Read as _;
                flate2::read::GzDecoder::new(decoded.as_slice())
                    .read_to_end(&mut bytes)
                    .unwrap();
            } else {
                bytes = decoded;
            }
            serde_json::from_slice(&bytes).unwrap()
        })
        .collect();
    assert!(fragments.iter().any(
        |fragment| fragment["systemd"]["units"]
            .as_array()
            .is_some_and(|units| units.iter().any(|unit| unit["name"]
                == "ucore-bootstrap.service"
                && unit["enabled"] == true
                && unit["contents"]
                    .as_str()
                    .is_some_and(|text| text.contains("ucore-rebase.sh"))))
    ));
    assert!(bootstrap["dropins"][0]["contents"]
        .as_str()
        .unwrap()
        .contains("Requires=skillet-tpm-bootstrap.service"));
    assert!(units
        .iter()
        .any(|u| u["name"] == "skillet-tpm-bootstrap.service"));
    assert!(ignition["storage"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "/etc/skillet-install/ublue.der"));
}
