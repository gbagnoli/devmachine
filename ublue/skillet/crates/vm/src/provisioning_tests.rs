use super::*;
use crate::{Backend, Connection};
use std::{fs, net::TcpListener, os::unix::fs::symlink, path::PathBuf};

#[test]
fn ssh_forward_port_preflight_rejects_invalid_and_occupied_ports() {
    assert!(validate_ssh_port(2199).is_err());
    assert!(validate_ssh_port(2300).is_err());
    let (port, listener) = (2200..=2299)
        .find_map(|port| {
            TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .ok()
                .map(|listener| (port, listener))
        })
        .expect("at least one test SSH port should be available");
    assert!(validate_ssh_port(port).is_err());
    drop(listener);
}

#[test]
fn image_resolution_requires_one_regular_image_or_an_explicit_selection() {
    let temp = tempfile::tempdir().unwrap();
    let images = temp.path().join("images");
    fs::create_dir(&images).unwrap();
    let first = images.join("fedora-coreos-44.1-qemu.x86_64.qcow2");
    fs::write(&first, "image").unwrap();
    assert_eq!(resolve_coreos_image(&images, None).unwrap(), first);

    let second = images.join("fedora-coreos-44.2-qemu.x86_64.qcow2");
    fs::write(&second, "image").unwrap();
    assert!(resolve_coreos_image(&images, None).is_err());
    assert_eq!(
        resolve_coreos_image(&images, Some(&second)).unwrap(),
        second
    );
}

#[test]
fn image_resolution_rejects_a_matching_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let images = temp.path().join("images");
    fs::create_dir(&images).unwrap();
    let target = temp.path().join("image.qcow2");
    fs::write(&target, "image").unwrap();
    symlink(&target, images.join("fedora-coreos-44.1-qemu.x86_64.qcow2")).unwrap();
    assert!(resolve_coreos_image(&images, None).is_err());
}

fn preparing_run() -> (tempfile::TempDir, ManifestStore, RunIdentity, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(temp.path(), crate::current_uid()).unwrap();
    let identity = RunIdentity::new("fixture", "provisioning").unwrap();
    let runtime_dir = PathBuf::from(format!("/run/user/{}", crate::current_uid()));
    store
        .prepare_intent(
            &identity,
            Connection {
                backend: Backend::Native,
                uri: "qemu:///session".into(),
                runtime_dir,
            },
            2204,
            "test-revision",
        )
        .unwrap();
    let image = temp.path().join("source.qcow2");
    fs::write(&image, b"disposable image bytes").unwrap();
    (temp, store, identity, image)
}

#[test]
fn local_artifacts_are_created_and_matching_retries_are_idempotent() {
    let (_temp, store, identity, image) = preparing_run();
    let run = store.load(&identity).unwrap();
    let key = prepare_local_artifacts(&store, &identity, &image).unwrap();
    assert!(key.starts_with("ssh-ed25519 "));
    assert_eq!(fs::read(&run.disk).unwrap(), b"disposable image bytes");
    assert_eq!(
        fs::metadata(&run.ssh.identity)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::read_to_string(run.ssh.identity.with_extension("pub"))
            .unwrap()
            .trim(),
        key
    );
    assert_eq!(
        fs::read_to_string(store.run_dir(&identity).join("fcos-image.sha256"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .len(),
        64
    );
    let repeated_key = prepare_local_artifacts(&store, &identity, &image).unwrap();
    assert_eq!(repeated_key, key);
    assert_eq!(fs::read(&run.disk).unwrap(), b"disposable image bytes");
}

#[test]
fn changed_image_replaces_only_the_preparing_run_disk() {
    let (_temp, store, identity, image) = preparing_run();
    let run = store.load(&identity).unwrap();
    prepare_local_artifacts(&store, &identity, &image).unwrap();
    fs::write(&image, b"replacement image").unwrap();
    prepare_local_artifacts(&store, &identity, &image).unwrap();
    assert_eq!(fs::read(run.disk).unwrap(), b"replacement image");
}

#[test]
fn incomplete_key_pair_and_symlinked_images_are_refused() {
    let (_temp, store, identity, image) = preparing_run();
    let run = store.load(&identity).unwrap();
    fs::create_dir(run.ssh.identity.parent().unwrap()).unwrap();
    fs::write(&run.ssh.identity, b"partial").unwrap();
    assert!(prepare_local_artifacts(&store, &identity, &image).is_err());

    let link = image.with_file_name("linked.qcow2");
    symlink(&image, &link).unwrap();
    assert!(prepare_disk(&store, &run, &link).is_err());
}

#[test]
fn artifact_preparation_refuses_progressed_runs() {
    let (_temp, store, identity, image) = preparing_run();
    let run = store.load(&identity).unwrap();
    let dir = store.run_dir(&identity);
    fs::create_dir_all(run.ssh.identity.parent().unwrap()).unwrap();
    fs::write(
        dir.join("skillet.sha256"),
        format!("{} host\n", "a".repeat(64)),
    )
    .unwrap();
    fs::write(
        dir.join("skillet-generic.sha256"),
        format!("{} generic\n", "b".repeat(64)),
    )
    .unwrap();
    store.mark_started(&identity).unwrap();
    assert!(prepare_local_artifacts(&store, &identity, &image).is_err());
}
