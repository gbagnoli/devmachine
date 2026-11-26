//! Optional network coordinator and authenticated broker for Home Assistant.
use super::Error;
use ring::pbkdf2;
use serde::{Deserialize, Serialize};
use skillet_core::{
    files::{FileMutationResource, FileReadResource, OwnerIdentity, Ownership},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource},
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PortProtocol, PortPublication,
    ProcessIdentity, QuadletSecret, SecretTarget, Volume,
};
use std::{collections::BTreeMap, net::SocketAddrV4, num::NonZeroU32, path::Path};
use zeroize::Zeroizing;

pub const FRONTEND_PORT: u16 = 8080;
pub const FRONTEND_LOOPBACK_PORT: u16 = 8099;
pub const CREDENTIAL: &str = "homeassistant_zigbee";
pub const UNIT: &str = "skillet-homeassistant-zigbee.service";
const NETWORK: &str = "homeassistant-mqtt";
const MQTT_DATA: &str = "/var/lib/data/mosquitto";
const ZIGBEE_DATA: &str = "/var/lib/data/zigbee2mqtt";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub coordinator: SocketAddrV4,
    pub ha_password: String,
    pub zigbee_password: String,
}

pub fn valid_address(value: &str) -> bool {
    value.parse::<SocketAddrV4>().is_ok_and(|address| {
        address.port() != 0 && !address.ip().is_unspecified() && !address.ip().is_multicast()
    })
}

pub fn valid_password(value: &str) -> bool {
    (32..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
}

pub fn install<S: ServiceResource + ?Sized, F: FileMutationResource + ?Sized>(
    system: &S,
    files: &F,
) -> Result<(), Error> {
    let content = format!(
        "[Unit]\nDescription=Apply Home Assistant MQTT and Zigbee2MQTT\nAfter=skillet-data-prepare.service\nRequiresMountsFor=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/{CREDENTIAL}.cred\n[Service]\nType=oneshot\nLoadCredentialEncrypted={CREDENTIAL}:/etc/credstore.encrypted/skillet/{CREDENTIAL}.cred\nExecStart=/usr/bin/skillet homeassistant zigbee\nTimeoutStartSec=900\n[Install]\nWantedBy=multi-user.target\n"
    );
    let root = Ownership::named(Some("root"), Some("root"));
    if files.ensure_file(
        &Path::new("/etc/systemd/system").join(UNIT),
        content.as_bytes(),
        Some(0o644),
        &root,
    )? {
        system.daemon_reload()?;
    }
    system.service_enable(UNIT)?;
    Ok(())
}

fn password_record(user: &str, password: &str) -> Result<String, Error> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use sha2::{Digest, Sha256};
    // Derive a stable per-user salt from the high-entropy generated credential.
    let digest = Sha256::new()
        .chain_update(b"skillet-mosquitto-salt:")
        .chain_update(user)
        .chain_update(b":")
        .chain_update(password)
        .finalize();
    let salt = &digest[..12];
    let rounds = NonZeroU32::new(20_000).ok_or(Error::InvalidZigbee)?;
    let mut hash = [0; 64];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA512,
        rounds,
        salt,
        password.as_bytes(),
        &mut hash,
    );
    Ok(format!(
        "{user}:$7${rounds}${}${}\n",
        STANDARD.encode(salt),
        STANDARD.encode(hash)
    ))
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    payload: &str,
    frontend_network: &skillet_podman::PodmanNetwork,
) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let input: Settings = serde_json::from_str(payload).map_err(|_| Error::InvalidZigbee)?;
    let ha_password = Zeroizing::new(input.ha_password);
    let zigbee_password = Zeroizing::new(input.zigbee_password);
    if !valid_address(&input.coordinator.to_string())
        || !valid_password(&ha_password)
        || !valid_password(&zigbee_password)
        || *ha_password == *zigbee_password
    {
        return Err(Error::InvalidZigbee);
    }
    apply_broker(system, files, &ha_password, &zigbee_password)?;
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new(ZIGBEE_DATA), Some(0o700), &root)?;
    let seed = Path::new(ZIGBEE_DATA).join("configuration.yaml");
    if files.read_file(&seed)?.is_none() {
        files.ensure_file(&seed, b"version: 5\nmqtt:\n  password: '!skillet-mqtt.yaml password'\nadvanced:\n  network_key: GENERATE\n  pan_id: GENERATE\n  ext_pan_id: GENERATE\n  enable_external_js: false\n", Some(0o600), &root)?;
    }
    let mqtt_secret = Zeroizing::new(format!("password: '{}'", *zigbee_password));
    system.ensure_podman_secret("skillet-zigbee-mqtt", &mqtt_secret)?;
    skillet_podman::ensure_network(system, files, frontend_network)?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "zigbee2mqtt".into(),
            image: "ghcr.io/koenkk/zigbee2mqtt:2.14.3".into(),
            network_attachments: vec![
                NetworkAttachment::Bridge(NETWORK.into()),
                NetworkAttachment::Bridge(frontend_network.unit_name.clone()),
            ],
            port_publications: vec![PortPublication {
                host_address: std::net::Ipv4Addr::LOCALHOST.into(),
                host_port: FRONTEND_LOOPBACK_PORT,
                container_port: FRONTEND_PORT,
                protocol: PortProtocol::Tcp,
            }],
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![volume(ZIGBEE_DATA, "/app/data", "Z")],
            secrets: vec![QuadletSecret {
                secret_name: "skillet-zigbee-mqtt".into(),
                target: SecretTarget::File {
                    target_path: "/app/data/skillet-mqtt.yaml".into(),
                    mode: Some("0400".into()),
                    uid: None,
                    gid: None,
                },
            }],
            config_revisions: vec![],
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=zigbee2mqtt".into(),
                        format!(
                            "Environment=ZIGBEE2MQTT_CONFIG_SERIAL_PORT=tcp://{}",
                            input.coordinator
                        ),
                        "Environment=ZIGBEE2MQTT_CONFIG_SERIAL_ADAPTER=zstack".into(),
                        "Environment=ZIGBEE2MQTT_CONFIG_MQTT_SERVER=mqtt://mosquitto:1883".into(),
                        "Environment=ZIGBEE2MQTT_CONFIG_MQTT_USER=zigbee2mqtt".into(),
                    "Environment=\"ZIGBEE2MQTT_CONFIG_MQTT_PASSWORD=!skillet-mqtt.yaml password\"".into(),
                        "Environment=ZIGBEE2MQTT_CONFIG_HOMEASSISTANT_ENABLED=true".into(),
                        "Environment=ZIGBEE2MQTT_CONFIG_FRONTEND_ENABLED=true".into(),
                    ],
                ),
                (
                    "Unit".into(),
                    vec![
                        "Requires=mosquitto.service".into(),
                        "After=mosquitto.service".into(),
                    ],
                ),
                (
                    "Service".into(),
                    vec!["Restart=on-failure".into(), "RestartSec=15s".into()],
                ),
            ]),
        },
    )?;
    skillet_datadog::configure_zigbee_checks(
        system,
        files,
        FRONTEND_LOOPBACK_PORT,
        *input.coordinator.ip(),
    )?;
    Ok(())
}

fn apply_broker<S, F>(
    system: &S,
    files: &F,
    ha_password: &str,
    zigbee_password: &str,
) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let broker_user = Ownership {
        uid: Some(OwnerIdentity::Id(1883)),
        gid: Some(OwnerIdentity::Id(1883)),
    };
    let config = Path::new("/etc/skillet/homeassistant-mqtt");
    files.ensure_directory(config, Some(0o755), &root)?;
    files.ensure_directory(Path::new(MQTT_DATA), Some(0o700), &broker_user)?;
    let broker_config = b"listener 1883\nallow_anonymous false\npassword_file /mosquitto/config/passwords\nacl_file /mosquitto/config/acl\npersistence true\npersistence_location /mosquitto/data/\nlog_dest stdout\n";
    let acl = b"user homeassistant\ntopic read homeassistant/#\ntopic write homeassistant/status\ntopic readwrite zigbee2mqtt/#\nuser zigbee2mqtt\ntopic readwrite zigbee2mqtt/#\ntopic write homeassistant/#\ntopic read homeassistant/status\n";
    let passwords = format!(
        "{}{}",
        password_record("homeassistant", ha_password)?,
        password_record("zigbee2mqtt", zigbee_password)?
    );
    for (name, content) in [
        ("mosquitto.conf", broker_config.as_slice()),
        ("acl", acl.as_slice()),
        ("passwords", passwords.as_bytes()),
    ] {
        files.ensure_file(&config.join(name), content, Some(0o600), &broker_user)?;
    }
    skillet_podman::ensure_network(system, files, &skillet_podman::isolated_network(NETWORK))?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "mosquitto".into(),
            image: "docker.io/library/eclipse-mosquitto:2.0.22".into(),
            network_attachments: vec![NetworkAttachment::Bridge(NETWORK.into())],
            port_publications: vec![PortPublication {
                host_address: std::net::Ipv4Addr::LOCALHOST.into(),
                host_port: 1883,
                container_port: 1883,
                protocol: PortProtocol::Tcp,
            }],
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::Numeric {
                uid: 1883,
                gid: 1883,
            },
            namespace_mapping: None,
            volumes: vec![
                volume(
                    "/etc/skillet/homeassistant-mqtt",
                    "/mosquitto/config",
                    "ro,Z",
                ),
                volume(MQTT_DATA, "/mosquitto/data", "Z"),
            ],
            secrets: vec![],
            config_revisions: vec![broker_config.to_vec(), acl.to_vec(), passwords.into_bytes()],
            extra_config: BTreeMap::from([
                ("Container".into(), vec!["ContainerName=mosquitto".into()]),
                ("Service".into(), vec!["Restart=on-failure".into()]),
            ]),
        },
    )?;
    Ok(())
}

fn volume(host: &str, target: &str, options: &str) -> Volume {
    Volume {
        host_path: host.into(),
        container_path: target.into(),
        options: Some(options.into()),
        host_mode: None,
        host_ownership: None,
    }
}

#[cfg(test)]
#[path = "zigbee_tests.rs"]
mod tests;
