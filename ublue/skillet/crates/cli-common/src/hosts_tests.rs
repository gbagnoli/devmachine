use super::{clamps_tailscale_config, TAILSCALE_AUTH_KEY_CREDENTIAL};
use skillet_podman::SecretTarget;

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
