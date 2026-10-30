//! Private native host networking. Preparation never reloads or activates a connection.
use serde::{Deserialize, Serialize};
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, OwnerIdentity, Ownership},
    system::{ServiceResource, SystemError},
};
use std::{fmt::Write as _, net::Ipv4Addr, path::Path};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub const CREDENTIAL: &str = "network_config";
pub const PREPARE_UNIT: &str = "skillet-network-prepare.service";
pub const CONNECTION_DIR: &str = "/run/NetworkManager/system-connections";

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("invalid private network configuration; check interfaces, addresses, subnet, DNS and Wi-Fi fields")]
    Invalid,
    #[error("network payload does not match the caller's host or interface declaration")]
    Declaration,
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
}

/// Caller selects interfaces; private values never enable a capability implicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    pub wired: bool,
    pub wifi: bool,
}

// No Debug: these types contain private deployment values and Wi-Fi credentials.
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub version: u8,
    pub host: String,
    pub wired: Option<Interface>,
    pub wifi: Option<Wireless>,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    pub interface: String,
    pub address: String,
    pub prefix: u8,
    pub gateway: String,
    pub dns: [String; 2],
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Wireless {
    pub network: Interface,
    pub ssid: String,
    pub password: String,
}

pub fn valid_interface(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 15
        && !matches!(value, "lo" | "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn valid_ipv4(value: &str) -> bool {
    value
        .parse::<Ipv4Addr>()
        .is_ok_and(|ip| !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast())
}

/// Accept a prefix or a contiguous dotted mask. This profile needs a LAN gateway.
pub fn prefix_from_mask(value: &str) -> Option<u8> {
    if let Ok(prefix) = value.strip_prefix('/').unwrap_or(value).parse::<u8>() {
        return (1..=30).contains(&prefix).then_some(prefix);
    }
    let mask = u32::from(value.parse::<Ipv4Addr>().ok()?);
    let prefix = u8::try_from(mask.count_ones()).ok()?;
    ((1..=30).contains(&prefix) && mask == u32::MAX.checked_shl(32 - u32::from(prefix))?)
        .then_some(prefix)
}

pub fn valid_ssid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32 && !value.contains('\0')
}

pub fn valid_wifi_key(value: &str) -> bool {
    ((8..=63).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_graphic() || b == b' '))
        || (value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Interface {
    fn validate(&self) -> Result<(), NetworkError> {
        if !valid_interface(&self.interface)
            || !(1..=30).contains(&self.prefix)
            || !valid_ipv4(&self.address)
            || !valid_ipv4(&self.gateway)
            || self.dns.iter().any(|value| !valid_ipv4(value))
        {
            return Err(NetworkError::Invalid);
        }
        let address = u32::from(
            self.address
                .parse::<Ipv4Addr>()
                .map_err(|_| NetworkError::Invalid)?,
        );
        let gateway = u32::from(
            self.gateway
                .parse::<Ipv4Addr>()
                .map_err(|_| NetworkError::Invalid)?,
        );
        let mask = u32::MAX << (32 - self.prefix);
        let network = address & mask;
        let broadcast = network | !mask;
        if address == gateway
            || gateway & mask != network
            || [address, gateway]
                .iter()
                .any(|ip| *ip == network || *ip == broadcast)
            || [&self.address, &self.gateway].iter().any(|value| {
                value
                    .parse::<Ipv4Addr>()
                    .is_ok_and(|ip| ip.is_loopback() || ip.is_link_local())
            })
        {
            return Err(NetworkError::Invalid);
        }
        Ok(())
    }
}

impl Input {
    pub fn parse(payload: &str, host: &str, selection: Selection) -> Result<Self, NetworkError> {
        let input: Self = serde_json::from_str(payload).map_err(|_| NetworkError::Invalid)?;
        input.validate(host, selection)?;
        Ok(input)
    }

    pub fn validate(&self, host: &str, selection: Selection) -> Result<(), NetworkError> {
        if self.version != 1
            || self.host != host
            || self.wired.is_some() != selection.wired
            || self.wifi.is_some() != selection.wifi
            || (!selection.wired && !selection.wifi)
        {
            return Err(NetworkError::Declaration);
        }
        if let Some(wired) = &self.wired {
            wired.validate()?;
        }
        if let Some(wifi) = &self.wifi {
            wifi.network.validate()?;
            if !valid_ssid(&wifi.ssid) || !valid_wifi_key(&wifi.password) {
                return Err(NetworkError::Invalid);
            }
        }
        if let (Some(wired), Some(wifi)) = (&self.wired, &self.wifi) {
            if wired.interface == wifi.network.interface || wired.address == wifi.network.address {
                return Err(NetworkError::Invalid);
            }
        }
        Ok(())
    }

    pub fn payload(
        &self,
        host: &str,
        selection: Selection,
    ) -> Result<Zeroizing<String>, NetworkError> {
        self.validate(host, selection)?;
        serde_json::to_string(self)
            .map(Zeroizing::new)
            .map_err(|_| NetworkError::Invalid)
    }
}

fn root() -> Ownership {
    Ownership {
        uid: Some(OwnerIdentity::Id(0)),
        gid: Some(OwnerIdentity::Id(0)),
    }
}

/// Public unit definitions only. No credential needed for bootstrap DHCP.
pub fn install<S, F>(system: &S, files: &F) -> Result<(), NetworkError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + ?Sized,
{
    let directory = Path::new("/etc/systemd/system");
    files.ensure_directory(directory, Some(0o755), &root())?;
    files.ensure_directory(
        &directory.join("NetworkManager.service.d"),
        Some(0o755),
        &root(),
    )?;
    let mut changed = files.ensure_file(
        &directory.join(PREPARE_UNIT),
        include_bytes!("prepare.service"),
        Some(0o644),
        &root(),
    )?;
    changed |= files.ensure_file(
        &directory.join("NetworkManager.service.d/skillet-network.conf"),
        include_bytes!("network-manager.conf"),
        Some(0o644),
        &root(),
    )?;
    if changed {
        system.daemon_reload()?;
    }
    Ok(())
}

/// Preserve DNS order and prefer Ethernet routes over Wi-Fi routes.
/// UUID omission deliberately uses `NetworkManager`'s deterministic filename hash.
pub fn render(
    network: &Interface,
    wifi: Option<&Wireless>,
    autoconnect: bool,
) -> Result<Zeroizing<String>, NetworkError> {
    network.validate()?;
    if wifi.is_some_and(|wifi| !valid_ssid(&wifi.ssid) || !valid_wifi_key(&wifi.password)) {
        return Err(NetworkError::Invalid);
    }
    let kind = if wifi.is_some() { "wifi" } else { "wired" };
    let metric = if wifi.is_some() { 600 } else { 100 };
    let connection_type = if wifi.is_some() { "wifi" } else { "ethernet" };
    let mut output = Zeroizing::new(format!("[connection]\nid=skillet-{kind}\ntype={connection_type}\ninterface-name={}\nautoconnect={autoconnect}\nautoconnect-priority=100\n", network.interface));
    if let Some(wifi) = wifi {
        output.push_str("\n[wifi]\nmode=infrastructure\ncloned-mac-address=permanent\nssid=");
        // Byte lists avoid ambiguity/injection for arbitrary UTF-8 SSIDs.
        for byte in wifi.ssid.bytes() {
            let _ = write!(output, "{byte};");
        }
        output.push_str("\n\n[wifi-security]\nkey-mgmt=wpa-psk\npsk-flags=0\npsk=");
        for character in wifi.password.chars() {
            match character {
                '\\' => output.push_str("\\\\"),
                ' ' => output.push_str("\\s"),
                _ => output.push(character),
            }
        }
        output.push('\n');
    }
    let _ = write!(output, "\n[ipv4]\nmethod=manual\naddress1={}/{}\ngateway={}\ndns={};{};\nignore-auto-dns=true\nroute-metric={metric}\n\n[ipv6]\nmethod=auto\nignore-auto-dns=true\nroute-metric={metric}\n", network.address, network.prefix, network.gateway, network.dns[0], network.dns[1]);
    Ok(output)
}

/// Boot prepares autoconnecting profiles before NM starts. Live first delivery
/// stages disabled profiles; retained boot profiles preserve their existing flag.
/// Never asks NM to reload, restart, reapply or activate any connection.
pub fn prepare<S, F>(
    system: &S,
    files: &F,
    input: &Input,
    host: &str,
    selection: Selection,
) -> Result<(), NetworkError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    input.validate(host, selection)?;
    let running = system.service_is_active("NetworkManager.service")?;
    let directory = Path::new(CONNECTION_DIR);
    files.ensure_directory(directory, Some(0o700), &root())?;
    for (kind, network, wifi) in [
        ("wired", input.wired.as_ref(), None),
        (
            "wifi",
            input.wifi.as_ref().map(|w| &w.network),
            input.wifi.as_ref(),
        ),
    ] {
        let path = directory.join(format!("skillet-{kind}.nmconnection"));
        if let Some(network) = network {
            let prior = files.read_file(&path)?.map(Zeroizing::new);
            let retained = prior.as_ref().is_some_and(|bytes| {
                bytes
                    .split(|b| *b == b'\n')
                    .any(|line| line == b"autoconnect=true")
            });
            let contents = render(network, wifi, !running || retained)?;
            files.ensure_file(&path, contents.as_bytes(), Some(0o600), &root())?;
        } else {
            // Explicit payload ownership permits removal of only our runtime files.
            files.delete_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
