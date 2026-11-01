//! Additive, source-scoped firewalld convergence for directly attached IPv4 LANs.
use crate::system::{LinuxSystemResource, SystemError};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io, net::Ipv4Addr, path::Path};
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedObjectPath,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FirewallProtocol {
    Tcp,
    Udp,
}

impl FirewallProtocol {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FirewallPort {
    pub port: u16,
    pub protocol: FirewallProtocol,
}

pub trait FirewallResource {
    /// Ensure runtime and permanent grants for up, hardware-backed NICs' RFC1918
    /// subnets. Does not reload firewalld or remove existing/shared grants.
    fn ensure_private_ipv4_lan_ports(&self, ports: &[FirewallPort]) -> Result<bool, SystemError>;
}

impl FirewallResource for LinuxSystemResource {
    fn ensure_private_ipv4_lan_ports(&self, ports: &[FirewallPort]) -> Result<bool, SystemError> {
        validate_ports(ports)?;
        if ports.is_empty() {
            return Ok(false);
        }
        let lans = observe_lans()?;
        let backend = DbusFirewall(Connection::system()?);
        converge(&backend, &lans, ports)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Lan {
    interface: String,
    subnet: String,
}

fn private_subnet(address: Ipv4Addr, mask: Ipv4Addr) -> Option<String> {
    if !address.is_private() {
        return None;
    }
    let mask = u32::from(mask);
    let prefix = mask.leading_ones();
    if !(1..=30).contains(&prefix) || u32::MAX.checked_shl(32 - prefix)? != mask {
        return None;
    }
    let network = Ipv4Addr::from(u32::from(address) & mask);
    let last = Ipv4Addr::from(u32::from(network) | !mask);
    // A private address with an overly broad mask must not grant public ranges.
    (network.is_private() && last.is_private()).then(|| format!("{network}/{prefix}"))
}

fn observe_lans() -> Result<Vec<Lan>, SystemError> {
    let mut lans = BTreeSet::new();
    for interface in nix::ifaddrs::getifaddrs().map_err(io::Error::from)? {
        if !interface
            .flags
            .contains(nix::net::if_::InterfaceFlags::IFF_UP)
        {
            continue;
        }
        let (Some(address), Some(mask)) = (interface.address, interface.netmask) else {
            continue;
        };
        let (Some(address), Some(mask)) = (address.as_sockaddr_in(), mask.as_sockaddr_in()) else {
            continue;
        };
        let Some(subnet) = private_subnet(address.ip(), mask.ip()) else {
            continue;
        };
        // Excludes loopback, container bridges/veths, and VPN/tunnel interfaces;
        // includes physical hardware and a VM's virtio NIC.
        match fs::metadata(
            Path::new("/sys/class/net")
                .join(&interface.interface_name)
                .join("device"),
        ) {
            Ok(_) => {
                lans.insert(Lan {
                    interface: interface.interface_name,
                    subnet,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(lans.into_iter().collect())
}

trait Firewalld {
    fn zone(&self, interface: &str) -> Result<String, SystemError>;
    fn contains(&self, zone: &str, rule: &str, permanent: bool) -> Result<bool, SystemError>;
    fn add(&self, zone: &str, rule: &str, permanent: bool) -> Result<(), SystemError>;
}

fn validate_ports(ports: &[FirewallPort]) -> Result<(), SystemError> {
    if ports.iter().any(|port| port.port == 0) {
        return Err(SystemError::Firewall("port zero is invalid".into()));
    }
    Ok(())
}

fn rule(lan: &Lan, port: FirewallPort) -> String {
    format!(
        "rule family=\"ipv4\" source address=\"{}\" port port=\"{}\" protocol=\"{}\" accept",
        lan.subnet,
        port.port,
        port.protocol.as_str()
    )
}

fn converge(
    backend: &impl Firewalld,
    lans: &[Lan],
    ports: &[FirewallPort],
) -> Result<bool, SystemError> {
    validate_ports(ports)?;
    if ports.is_empty() {
        return Ok(false);
    }
    if lans.is_empty() {
        return Err(SystemError::Firewall(
            "no active, directly attached private IPv4 LAN subnet".into(),
        ));
    }
    let mut desired = BTreeSet::new();
    for lan in lans {
        let zone = backend.zone(&lan.interface)?;
        if zone.is_empty() {
            return Err(SystemError::Firewall(
                "host interface has no resolvable firewall zone".into(),
            ));
        }
        for port in ports {
            desired.insert((zone.clone(), rule(lan, *port)));
        }
    }
    let mut changed = false;
    for (zone, rule) in desired {
        // Persist before activation so an interrupted apply cannot leave a
        // runtime-only opening that silently disappears at reboot.
        for permanent in [true, false] {
            if !backend.contains(&zone, &rule, permanent)? {
                backend.add(&zone, &rule, permanent)?;
                changed = true;
            }
        }
    }
    Ok(changed)
}

struct DbusFirewall(Connection);
const DESTINATION: &str = "org.fedoraproject.FirewallD1";
const ROOT: &str = "/org/fedoraproject/FirewallD1";

impl DbusFirewall {
    fn runtime(&self) -> Result<Proxy<'_>, SystemError> {
        Ok(Proxy::new(
            &self.0,
            DESTINATION,
            ROOT,
            "org.fedoraproject.FirewallD1.zone",
        )?)
    }
    fn permanent(&self, zone: &str) -> Result<Proxy<'_>, SystemError> {
        let config = Proxy::new(
            &self.0,
            DESTINATION,
            "/org/fedoraproject/FirewallD1/config",
            "org.fedoraproject.FirewallD1.config",
        )?;
        let path: OwnedObjectPath = config.call("getZoneByName", &(zone,))?;
        Ok(Proxy::new(
            &self.0,
            DESTINATION,
            path,
            "org.fedoraproject.FirewallD1.config.zone",
        )?)
    }
}

impl Firewalld for DbusFirewall {
    fn zone(&self, interface: &str) -> Result<String, SystemError> {
        let zone: String = self.runtime()?.call("getZoneOfInterface", &(interface,))?;
        if !zone.is_empty() {
            return Ok(zone);
        }
        let root = Proxy::new(&self.0, DESTINATION, ROOT, DESTINATION)?;
        Ok(root.call("getDefaultZone", &())?)
    }
    fn contains(&self, zone: &str, rule: &str, permanent: bool) -> Result<bool, SystemError> {
        if permanent {
            Ok(self.permanent(zone)?.call("queryRichRule", &(rule,))?)
        } else {
            Ok(self.runtime()?.call("queryRichRule", &(zone, rule))?)
        }
    }
    fn add(&self, zone: &str, rule: &str, permanent: bool) -> Result<(), SystemError> {
        if permanent {
            self.permanent(zone)?.call_method("addRichRule", &(rule,))?;
        } else {
            self.runtime()?
                .call_method("addRichRule", &(zone, rule, 0i32))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "firewall/tests.rs"]
mod tests;
