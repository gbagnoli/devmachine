//! Read-only native-auth HTTPS probes. These isolate TLS/proxy behavior inside
//! the owned guest; they do not establish reachability from the public internet.
use super::{caddy_container_address, CaddySites, Duration, GuestTransport, UiProvisioningError};

#[derive(Clone, Copy)]
enum Probe {
    JoplinPing,
    JoplinAuth,
    JellyfinInfo,
    JellyfinAuth,
}

impl Probe {
    fn path(self) -> &'static str {
        match self {
            Self::JoplinPing => "/api/ping",
            Self::JoplinAuth => "/api/users",
            Self::JellyfinInfo => "/System/Info/Public",
            Self::JellyfinAuth => "/Users/Me",
        }
    }
    fn accepts(self, body: &str, status: &str) -> bool {
        if matches!(self, Self::JellyfinAuth) {
            return status == "401";
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
            return false;
        };
        match self {
            Self::JoplinPing => status == "200" && value["status"] == "ok",
            Self::JoplinAuth => matches!(status, "401" | "403") && value.get("error").is_some(),
            Self::JellyfinInfo => {
                status == "200"
                    && value["Version"].as_str().is_some_and(|v| !v.is_empty())
                    && value["ProductName"]
                        .as_str()
                        .is_some_and(|v| v.contains("Jellyfin"))
            }
            Self::JellyfinAuth => false,
        }
    }
}

pub fn verify_public_joplin(
    guest: &impl GuestTransport,
    sites: &CaddySites,
    timeout: Duration,
    interval: Duration,
) -> Result<(), UiProvisioningError> {
    verify(
        guest,
        sites,
        "joplin",
        &[Probe::JoplinPing, Probe::JoplinAuth],
        timeout,
        interval,
    )
}

/// Verify both the canonical hostname and every caller-declared alias.
pub fn verify_public_jellyfin(
    guest: &impl GuestTransport,
    sites: &CaddySites,
    timeout: Duration,
    interval: Duration,
) -> Result<(), UiProvisioningError> {
    verify(
        guest,
        sites,
        "jellyfin",
        &[Probe::JellyfinInfo, Probe::JellyfinAuth],
        timeout,
        interval,
    )
}

fn verify(
    guest: &impl GuestTransport,
    sites: &CaddySites,
    name: &str,
    probes: &[Probe],
    timeout: Duration,
    interval: Duration,
) -> Result<(), UiProvisioningError> {
    let Some(site) = sites
        .services
        .iter()
        .find(|site| site.name == name && site.access == skillet_caddy::SiteAccess::Public)
    else {
        return Ok(());
    };
    let address = caddy_container_address(guest, &sites.host)?;
    for hostname in std::iter::once(&site.hostname).chain(&site.aliases) {
        let resolve = format!("{hostname}:443:{address}");
        for probe in probes {
            let url = format!("https://{hostname}{}", probe.path());
            let started = std::time::Instant::now();
            loop {
                let mut arguments = vec![
                    "--silent",
                    "--show-error",
                    "--max-time",
                    "8",
                    "--max-filesize",
                    "1048576",
                    "--resolve",
                    &resolve,
                    "--header",
                    "X-Skillet-User: forged@example.invalid",
                    "--header",
                    "Tailscale-User: forged@example.invalid",
                    "--write-out",
                    "\\n%{http_code}",
                ];
                if sites.acme_staging {
                    arguments.push("--insecure");
                }
                arguments.push(&url);
                let output = guest.execute(
                    &skillet_vm::transport::GuestCommand {
                        program: "/usr/bin/curl",
                        arguments: &arguments,
                    },
                    None,
                )?;
                let accepted = output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .rsplit_once('\n')
                        .is_some_and(|(body, status)| probe.accepts(body, status));
                if accepted {
                    break;
                }
                if started.elapsed() >= timeout {
                    return Err(UiProvisioningError::Invalid(format!("public {name} HTTPS/native-auth probe failed for {} (response bodies suppressed)",probe.path())));
                }
                std::thread::sleep(interval);
            }
        }
    }
    Ok(())
}
