use super::{clamps_tailscale_config, ui_config_for_host, TAILSCALE_AUTH_KEY_CREDENTIAL};
use skillet_podman::SecretTarget;

#[test]
fn boot_expectations_are_profile_inputs_and_unknown_profiles_are_refused() {
    let clamps = super::boot_policy_for_host("clamps").unwrap();
    assert_eq!(clamps.signed_image, "ghcr.io/gbagnoli/ucore-clamps");
    assert_eq!(clamps.masked_units, ["systemd-resolved.service"]);
    let other = super::boot_policy_for_host("beezelbot").unwrap();
    assert!(other.masked_units.is_empty());
    assert!(super::boot_policy_for_host("clamps-test-smoke").is_none());
    assert!(super::boot_policy_for_host("unknown").is_none());
}

#[test]
fn clamps_tailscale_uses_host_network_and_persistent_state() {
    let config = clamps_tailscale_config("clamps-test-smoke", "test-auth-key".to_string());

    assert_eq!(config.name, "tailscale");
    assert_eq!(config.image, "docker.io/tailscale/tailscale:stable");
    assert!(config.networks.is_empty());
    assert_eq!(config.volumes.len(), 1);
    assert_eq!(config.volumes[0].host_path, "/var/lib/data/tailscale");
    assert_eq!(config.volumes[0].container_path, "/var/lib/tailscale");
    let container = &config.extra_config["Container"];
    assert!(container.contains(&"ContainerName=tailscale".to_string()));
    assert!(container.contains(&"Network=host".to_string()));
    assert!(container.contains(&"AddCapability=NET_ADMIN".to_string()));
    assert!(container.contains(&"AddCapability=NET_RAW".to_string()));
    assert!(container.contains(&"AddDevice=/dev/net/tun:/dev/net/tun".to_string()));
    assert!(container.contains(&"Environment=TS_AUTH_ONCE=true".to_string()));
    assert!(container.contains(&"Environment=TS_ACCEPT_DNS=false".to_string()));
    assert!(container.contains(&"Environment=TS_HOSTNAME=clamps-test-smoke".to_string()));
    assert!(config.secrets.iter().any(|secret| {
        secret.secret_name == TAILSCALE_AUTH_KEY_CREDENTIAL
            && matches!(
                &secret.target,
                SecretTarget::Environment { env_var_name } if env_var_name == "TS_AUTHKEY"
            )
    }));
}

#[test]
fn host_ui_declarations_include_only_the_services_each_host_runs() {
    let clamps = ui_config_for_host("clamps").expect("clamps UI declaration");
    assert_eq!(
        clamps
            .services
            .iter()
            .map(|service| service.name.as_str())
            .collect::<Vec<_>>(),
        ["pihole", "syncthing"]
    );
    assert_eq!(clamps.network.unit_name, "clamps");

    let beezelbot = ui_config_for_host("beezelbot").expect("beezelbot UI declaration");
    assert_eq!(beezelbot.services.len(), 1);
    assert_eq!(beezelbot.services[0].name, "syncthing");
    assert_eq!(beezelbot.network.unit_name, "beezelbot");
    assert!(ui_config_for_host("unknown-host").is_none());
}
