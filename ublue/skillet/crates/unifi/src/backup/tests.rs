use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::{os::unix::fs::symlink, sync::atomic::Ordering};

fn archive(root: &Path, name: &str, age: u64) -> PathBuf {
    let path = root.join(name);
    fs::write(&path, b"opaque-native-unifi-backup").unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(age)))
        .unwrap();
    path
}

fn run(source: &Path, destination: &Path) -> Result<Outcome, UnifiError> {
    export(
        source,
        destination,
        nix::unistd::getuid().as_raw(),
        nix::unistd::getgid().as_raw(),
        SystemTime::now(),
    )
}

#[test]
fn publishes_stable_archives_privately_and_repeat_is_idempotent() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let backup = archive(source.path(), "autobackup_1.unf", 120);
    archive(source.path(), "autobackup_writing.unf", 0);
    fs::write(source.path().join("autobackup_partial.unf.tmp"), b"partial").unwrap();
    assert_eq!(run(source.path(), destination.path()).unwrap().published, 1);
    let published = destination.path().join(backup.file_name().unwrap());
    assert_eq!(fs::read(&published).unwrap(), fs::read(&backup).unwrap());
    assert_eq!(
        fs::metadata(&published).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&published).unwrap().uid(),
        nix::unistd::getuid().as_raw()
    );
    assert_eq!(run(source.path(), destination.path()).unwrap().published, 0);
}

#[test]
fn retains_seven_and_preserves_unrelated_files() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    for index in 0..9 {
        archive(
            source.path(),
            &format!("autobackup_{index}.unf"),
            120 + index,
        );
    }
    archive(destination.path(), "autobackup_old.unf", 3600);
    fs::write(destination.path().join("manual.unf"), b"keep").unwrap();
    let outcome = run(source.path(), destination.path()).unwrap();
    assert_eq!(outcome.published, 7);
    assert_eq!(outcome.pruned, 1);
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 8);
    assert!(destination.path().join("manual.unf").is_file());
    assert!(!destination.path().join("autobackup_old.unf").exists());
}

#[test]
fn conflicts_and_symlinks_fail_without_pruning_or_overwriting() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    archive(source.path(), "autobackup_1.unf", 120);
    fs::write(destination.path().join("autobackup_1.unf"), b"different").unwrap();
    let old = archive(destination.path(), "autobackup_old.unf", 3600);
    assert!(run(source.path(), destination.path()).is_err());
    assert!(old.is_file());
    fs::remove_file(destination.path().join("autobackup_1.unf")).unwrap();
    symlink(&old, destination.path().join("autobackup_1.unf")).unwrap();
    assert!(run(source.path(), destination.path()).is_err());
    fs::remove_file(destination.path().join("autobackup_1.unf")).unwrap();
    fs::remove_file(source.path().join("autobackup_1.unf")).unwrap();
    symlink(&old, source.path().join("autobackup_1.unf")).unwrap();
    assert!(run(source.path(), destination.path()).is_err());
}

#[test]
fn timer_is_repeatable_and_does_not_start_an_export_during_apply() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let config = Config {
        files_root: "/var/lib/data/syncthing/data",
        owner: "giacomo",
        group: "giacomo",
    };
    install(&system, &files, config).unwrap();
    let starts = system.start_count.load(Ordering::SeqCst);
    install(&system, &files, config).unwrap();
    assert_eq!(system.start_count.load(Ordering::SeqCst), starts);
    assert!(!system.service_is_active(UNIT).unwrap());
    let units = image_units();
    assert!(units[0].content.contains("/usr/bin/skillet unifi export"));
    assert!(units[1].content.contains("03:30:00 Europe/Paris"));
}
