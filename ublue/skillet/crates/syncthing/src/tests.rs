use super::*;
use skillet_core::{
    system::{GroupIdentity, UserIdentity},
    test_utils::{MockFiles, MockSystem},
};

#[test]
fn syncthing_uses_persistent_data_and_shared_dns_network() {
    let system = MockSystem::new();
    system.user_identities.lock().unwrap().insert(
        "giacomo".to_string(),
        UserIdentity {
            name: "giacomo".to_string(),
            uid: 1042,
            primary_gid: 2047,
        },
    );
    system.group_identities.lock().unwrap().insert(
        "giacomo".to_string(),
        GroupIdentity {
            name: "giacomo".to_string(),
            gid: 2047,
        },
    );
    let files = MockFiles::new();
    setup_data_mount(&files);
    files.files.lock().unwrap().insert(
        "/etc/subuid".to_string(),
        b"giacomo:100000:65536\n".to_vec(),
    );
    files.files.lock().unwrap().insert(
        "/etc/subgid".to_string(),
        b"giacomo:200000:65536\n".to_vec(),
    );

    apply(
        &system,
        &files,
        SyncthingConfig {
            data_path: "/var/lib/data/syncthing".to_string(),
            data_owner: "giacomo".to_string(),
            data_group: "giacomo".to_string(),
            container_uid: 1000,
            container_gid: 1000,
            network_name: "clamps".to_string(),
        },
    )
    .unwrap();

    let managed_files = files.files.lock().unwrap();
    let quadlet = managed_files
        .get("/etc/containers/systemd/syncthing.container")
        .unwrap();
    let quadlet = String::from_utf8_lossy(quadlet);
    for directive in [
        "AutoUpdate=registry",
        "ContainerName=syncthing",
        "User=1000:1000",
        "UIDMap=1000:1042:1",
        "GIDMap=1000:2047:1",
        "Network=clamps.network",
        "PublishPort=[::]:22000:22000/tcp",
        "PublishPort=0.0.0.0:22000:22000/tcp",
        "PublishPort=[::]:22000:22000/udp",
        "PublishPort=0.0.0.0:22000:22000/udp",
        "Volume=/var/lib/data/syncthing:/var/syncthing:z",
    ] {
        assert!(quadlet.contains(directive), "missing directive {directive}");
    }
    assert!(!quadlet.contains("Environment=PGID="));
    assert!(!quadlet.contains("Environment=PUID="));
    assert!(
        !quadlet
            .lines()
            .any(|line| line.starts_with("PublishPort=") && line.contains("8384")),
        "the GUI must only be reachable through the private reverse proxy"
    );
}

#[test]
fn syncthing_requires_declared_host_account_identities() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    setup_data_mount(&files);

    let result = apply(
        &system,
        &files,
        SyncthingConfig {
            data_path: "/var/lib/data/syncthing".to_string(),
            data_owner: "giacomo".to_string(),
            data_group: "giacomo".to_string(),
            container_uid: 1000,
            container_gid: 1000,
            network_name: "clamps".to_string(),
        },
    );

    assert!(matches!(result, Err(SyncthingError::UnknownDataOwner(name)) if name == "giacomo"));
    assert!(!files
        .directories
        .lock()
        .unwrap()
        .contains("/var/lib/data/syncthing"));
}

#[test]
fn syncthing_rejects_a_non_primary_host_group_for_namespace_mapping() {
    let system = MockSystem::new();
    system.user_identities.lock().unwrap().insert(
        "giacomo".to_string(),
        UserIdentity {
            name: "giacomo".to_string(),
            uid: 1042,
            primary_gid: 2047,
        },
    );
    system.group_identities.lock().unwrap().insert(
        "data".to_string(),
        GroupIdentity {
            name: "data".to_string(),
            gid: 2048,
        },
    );
    let files = MockFiles::new();
    setup_data_mount(&files);

    let result = apply(
        &system,
        &files,
        SyncthingConfig {
            data_path: "/var/lib/data/syncthing".to_string(),
            data_owner: "giacomo".to_string(),
            data_group: "data".to_string(),
            container_uid: 1000,
            container_gid: 1000,
            network_name: "clamps".to_string(),
        },
    );

    assert!(matches!(
        result,
        Err(SyncthingError::NonPrimaryDataGroup { owner, group })
            if owner == "giacomo" && group == "data"
    ));
    assert!(!files
        .directories
        .lock()
        .unwrap()
        .contains("/var/lib/data/syncthing"));
}

fn setup_data_mount(files: &MockFiles) {
    files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(
        std::path::Path::new("/var/lib/data"),
        "/dev/test",
        "/data",
        "btrfs",
    );
}
