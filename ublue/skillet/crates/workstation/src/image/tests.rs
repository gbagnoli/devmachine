use super::*;

fn binary() -> Vec<u8> {
    let mut bytes = vec![0; 120];
    bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
    bytes
}

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const VERSION: &str = "0.1.0+git.0123456789ab";

#[test]
fn rendering_is_deterministic_public_and_allowlisted() {
    let bytes = binary();
    let files = render("clamps", REVISION, VERSION, &bytes, &bytes).unwrap();
    let again = render("clamps", REVISION, VERSION, &bytes, &bytes).unwrap();
    assert_eq!(files.len(), 3);
    for (a, b) in files.iter().zip(again) {
        assert_eq!(a.bytes, b.bytes);
        assert!(a.path.starts_with("usr/"));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&files[2].bytes).unwrap();
    assert_eq!(manifest["source_commit"], REVISION);
    assert_eq!(
        manifest["artifacts"][0]["sha256"],
        hex::encode(Sha256::digest(&bytes))
    );
    assert!(manifest["services"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "btrbk"));
    assert!(manifest.get("network").is_none());
}

#[test]
fn rejects_dirty_mismatched_unknown_and_dynamic_inputs() {
    let bytes = binary();
    for version in [
        "0.1.0+git.0123456789ab.dirty",
        "0.1.0+git.unknown",
        "0.1.0+git.aaaaaaaaaaaa",
    ] {
        assert!(render("clamps", REVISION, version, &bytes, &bytes).is_err());
    }
    assert!(render("unknown", REVISION, VERSION, &bytes, &bytes).is_err());
    let mut dynamic = bytes.clone();
    dynamic[64..68].copy_from_slice(&3u32.to_le_bytes());
    assert!(render("clamps", REVISION, VERSION, &dynamic, &bytes).is_err());
    assert!(static_elf(b"not ELF").is_err());
}

#[test]
fn real_bundle_repeat_is_noop_and_conflicts_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("bundle");
    let bytes = binary();
    let files = render("clamps", REVISION, VERSION, &bytes, &bytes).unwrap();
    write(&output, &files).unwrap();
    write(&output, &files).unwrap();
    fs::write(output.join("unexpected"), b"preserve").unwrap();
    assert!(write(&output, &files).is_err());
    assert_eq!(fs::read(output.join("unexpected")).unwrap(), b"preserve");
}
