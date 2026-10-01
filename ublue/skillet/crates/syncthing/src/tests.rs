use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn syncthing_uses_persistent_data_and_shared_dns_network() {
    let system = MockSystem::new();
    let files = MockFiles::new();

    apply(
        &system,
        &files,
        SyncthingConfig {
            data_path: "/var/lib/data/syncthing".to_string(),
            data_owner: "giacomo".to_string(),
            data_group: "giacomo".to_string(),
            uid: 1000,
            gid: 1000,
            network: PodmanNetwork {
                unit_name: "clamps".to_string(),
                options: vec![
                    "DisableDNS=false".to_string(),
                    "Driver=bridge".to_string(),
                    "NetworkName=clamps".to_string(),
                    "Subnet=172.26.26.0/24".to_string(),
                ],
            },
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
        "Environment=PGID=1000",
        "Environment=PUID=1000",
        "Network=clamps.network",
        "PublishPort=[::]:22000:22000/tcp",
        "PublishPort=0.0.0.0:22000:22000/tcp",
        "PublishPort=[::]:22000:22000/udp",
        "PublishPort=0.0.0.0:22000:22000/udp",
        "Volume=/var/lib/data/syncthing:/var/syncthing:z",
    ] {
        assert!(quadlet.contains(directive), "missing directive {directive}");
    }
    assert!(
        !quadlet
            .lines()
            .any(|line| line.starts_with("PublishPort=") && line.contains("8384")),
        "the GUI must only be reachable through the private reverse proxy"
    );
}
