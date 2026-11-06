use super::*;
use std::os::unix::fs::MetadataExt;
#[test]
fn snapshot_writer_rejects_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", source.join("escape")).unwrap();
    let mut builder = tar::Builder::new(File::create(root.path().join("archive.tar")).unwrap());
    assert!(append_tree(&mut builder, &source, Path::new("state")).is_err());
}
#[test]
fn pruning_keeps_seven_owned_archives_and_ignores_temporary_and_foreign_files() {
    let root = tempfile::tempdir().unwrap();
    for number in 0..10 {
        let mut builder = tar::Builder::new(
            File::create(root.path().join(format!("gokapi-{number}.tar"))).unwrap(),
        );
        append_manifest(&mut builder).unwrap();
        builder.finish().unwrap();
    }
    for name in [
        ".syncthing.gokapi-01.tmp",
        "unrelated",
        "gokapi-foreign.tar",
    ] {
        fs::write(root.path().join(name), b"unrelated").unwrap();
    }
    prune(root.path(), 7).unwrap();
    assert!(!root.path().join("gokapi-2.tar").exists());
    assert!(root.path().join("gokapi-3.tar").exists());
    assert!(root.path().join("unrelated").exists());
}
#[test]
fn extraction_rejects_symlink_entries_without_touching_the_target() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("bad.tar");
    let mut builder = tar::Builder::new(File::create(&archive).unwrap());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o600);
    header.set_link_name("/etc/passwd").unwrap();
    header.set_cksum();
    builder
        .append_data(&mut header, "state", std::io::empty())
        .unwrap();
    builder.finish().unwrap();
    let staged = root.path().join("restore");
    fs::create_dir(&staged).unwrap();
    let owner = fs::metadata(&staged).unwrap();
    assert!(extract(&archive, &staged, owner.uid(), owner.gid()).is_err());
    assert!(!staged.join("state").exists());
}
