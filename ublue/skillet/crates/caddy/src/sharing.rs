//! Versioned public sharing surfaces. Everything else is denied before proxying.
use crate::{CaddyError, CaddySite, SiteAccess, UiService, UpstreamTransport};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Gokapi224,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub names: Vec<String>,
    pub policy: Policy,
}

/// Canonical identity of the public site owned by a private service declaration.
pub fn site_name(owner: &str) -> String {
    format!("{owner}-sharing")
}

pub(super) fn extend(
    sites: &mut Vec<CaddySite>,
    declarations: &[UiService],
    zone: Option<&str>,
    names: &mut BTreeMap<String, &str>,
) -> Result<(), CaddyError> {
    for declaration in declarations {
        let Some(sharing) = &declaration.sharing else {
            continue;
        };
        if !declaration.tailnet_identity
            || !declaration.public_names.is_empty()
            || declaration.transport != UpstreamTransport::Bridge
            || declaration.cache.is_some()
            || sharing.names.is_empty()
        {
            return Err(CaddyError::InvalidConfiguration(
                "sharing requires a private identity-protected bridge and public names".into(),
            ));
        }
        let zone = zone.ok_or_else(|| {
            CaddyError::InvalidConfiguration("sharing requires the Cloudflare zone".into())
        })?;
        let mut resolved = Vec::new();
        for name in &sharing.names {
            super::validate_label(name, "sharing label")?;
            let hostname = format!("{name}.{zone}");
            if names.insert(hostname.clone(), "sharing").is_some() {
                return Err(CaddyError::InvalidConfiguration(
                    "duplicate sharing hostname".into(),
                ));
            }
            resolved.push(hostname);
        }
        let name = site_name(&declaration.name);
        if sites.iter().any(|site| site.name == name) {
            return Err(CaddyError::InvalidConfiguration(
                "duplicate sharing service".into(),
            ));
        }
        let mut site = sites
            .iter()
            .find(|site| site.name == declaration.name)
            .cloned()
            .ok_or_else(|| CaddyError::InvalidConfiguration("sharing owner absent".into()))?;
        site.name = name;
        site.hostname = resolved.remove(0);
        site.aliases = resolved;
        site.tailnet_identity = false;
        site.anonymous_token_refresh = false;
        site.access = SiteAccess::Public;
        site.public_routes = Some(sharing.policy);
        sites.push(site);
    }
    Ok(())
}

/// The pinned upstream router has four guest-upload API endpoints. They retain
/// native request-key validation; administrative API keys are never accepted there.
pub fn render(policy: Policy, upstream: &str) -> String {
    match policy {
        Policy::Gokapi224 => format!(
            r#"        @encoded_path expression `{{http.request.orig_uri}}.matches('^[^?]*%')`
        respond @encoded_path 403
        @sharing_home {{
            method GET HEAD
            path /
        }}
        handle @sharing_home {{
            header Content-Type "text/plain; charset=utf-8"
            header Cache-Control no-store
            respond "File sharing endpoint. Open a complete download or upload link provided by the sender." 200
        }}
        @share_read {{
            method GET HEAD
            path /d /d/* /dh/* /downloadFile /publicUpload /h/* /hotlink/* /error /favicon.ico /site.webmanifest /main.wasm /e2e.wasm /assets/* /css/* /js/* /static/*
        }}
        @share_post {{
            method POST
            path /d /downloadFile /api/uploadrequest/chunk/add /api/uploadrequest/chunk/complete /api/uploadrequest/chunk/reserve /api/uploadrequest/chunk/unreserve
        }}
        handle @share_read {{
            reverse_proxy {upstream} {{
                header_up -Authorization
            }}
        }}
        handle @share_post {{
            reverse_proxy {upstream} {{
                header_up -Authorization
            }}
        }}
        respond "Public sharing route denied" 403
"#
        ),
    }
}

#[cfg(test)]
mod tests;
