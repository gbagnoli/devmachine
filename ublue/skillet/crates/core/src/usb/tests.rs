use super::*;
#[test]
fn sysfs_device_lookup_is_exact_and_handles_absent_devices() {
    let root = tempfile::tempdir().unwrap();
    let device = root.path().join("2-1");
    fs::create_dir(&device).unwrap();
    fs::write(device.join("idVendor"), "0764\n").unwrap();
    fs::write(device.join("idProduct"), "0501\n").unwrap();
    fs::create_dir(root.path().join("2-1:1.0")).unwrap();
    assert!(device_present(root.path(), 0x0764, 0x0501).unwrap());
    assert!(!device_present(root.path(), 0x0764, 0x0502).unwrap());
    assert!(!device_present(&root.path().join("absent"), 0x0764, 0x0501).unwrap());
    fs::write(device.join("idProduct"), "invalid").unwrap();
    assert!(device_present(root.path(), 0x0764, 0x0501).is_err());
}
