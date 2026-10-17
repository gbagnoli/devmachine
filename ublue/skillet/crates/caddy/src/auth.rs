//! Local, per-request tailnet identity authorization for Caddy `forward_auth`.
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::{
    collections::BTreeSet,
    io::Read as _,
    net::{IpAddr, SocketAddr},
    path::Path,
    time::Duration,
};
use thiserror::Error;

pub const USERS_ENTRY: &str = "skillet/tailscale/ui-users";
pub const DAEMON_SOCKET: &str = "/run/skillet-tailscale/tailscaled.sock";
pub const AUTH_SOCKET: &str = "/run/skillet-tailnet-ui/auth.sock";
pub const POLICY_FILE: &str = "/etc/skillet/tailnet-ui.json";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid tailnet UI policy; use distinct complete LoginName values, separated by commas or newlines")]
    InvalidPolicy,
    #[error("tailnet identity lookup failed")]
    Lookup,
    #[error("tailnet UI runtime I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not create tailnet UI authorization socket")]
    Listener,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    users: BTreeSet<String>,
}

impl Policy {
    pub fn new(users: Vec<String>) -> Result<Self, AuthError> {
        if users.iter().any(|name| !valid_identity(name)) {
            return Err(AuthError::InvalidPolicy);
        }
        let count = users.len();
        let unique: BTreeSet<_> = users.into_iter().collect();
        if unique.len() != count {
            return Err(AuthError::InvalidPolicy);
        }
        Ok(Self { users: unique })
    }

    pub fn parse_users(contents: &str) -> Result<Vec<String>, AuthError> {
        let users: Vec<_> = contents
            .split([',', '\n'])
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if users.is_empty() {
            return Err(AuthError::InvalidPolicy);
        }
        Self::new(users.clone())?;
        Ok(users)
    }

    pub fn authorize(&self, identity: &Identity) -> bool {
        identity.node.tags.is_empty()
            && identity.node.user != 0
            && identity.node.user == identity.user_profile.id
            && self.users.contains(&identity.user_profile.login_name)
    }
}

fn valid_identity(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 254
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b',')
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Identity {
    node: Node,
    user_profile: UserProfile,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    user: u64,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserProfile {
    #[serde(rename = "ID")]
    id: u64,
    login_name: String,
}

pub struct LocalApi {
    client: reqwest::blocking::Client,
}

impl LocalApi {
    pub fn new(socket: &Path) -> Result<Self, AuthError> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .unix_socket(socket)
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AuthError::Lookup)?;
        Ok(Self { client })
    }

    pub fn whois(&self, peer: SocketAddr) -> Result<Identity, AuthError> {
        let response = self
            .client
            .get("http://local-tailscaled.sock/localapi/v0/whois")
            .query(&[("addr", peer.to_string())])
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|_| AuthError::Lookup)?;
        let mut contents = Vec::new();
        response
            .take(65_537)
            .read_to_end(&mut contents)
            .map_err(|_| AuthError::Lookup)?;
        if contents.len() > 65_536 {
            return Err(AuthError::Lookup);
        }
        serde_json::from_slice(&contents).map_err(|_| AuthError::Lookup)
    }
}

fn tailnet_peer(peer: SocketAddr) -> bool {
    match peer.ip() {
        IpAddr::V4(ip) => ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]),
        IpAddr::V6(ip) => ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

/// This endpoint trusts only Caddy's private Unix socket connection. Caddy must
/// overwrite the peer header with the original TCP peer, never forwarded input.
pub fn authorize_peer(policy: &Policy, peer: &str, api: &LocalApi) -> Option<String> {
    let peer: SocketAddr = peer.parse().ok()?;
    if !tailnet_peer(peer) {
        return None;
    }
    let identity = api.whois(peer).ok()?;
    policy
        .authorize(&identity)
        .then(|| identity.user_profile.login_name.clone())
}

/// Guest runtime adapter. Policy is reread for every request; failures deny access.
pub fn serve(socket: &Path, daemon: &Path, policy_file: &Path) -> Result<(), AuthError> {
    use std::os::unix::{
        fs::{FileTypeExt as _, PermissionsExt as _},
        net::{UnixListener, UnixStream},
    };
    if let Ok(metadata) = std::fs::symlink_metadata(socket) {
        if !metadata.file_type().is_socket() || UnixStream::connect(socket).is_ok() {
            return Err(AuthError::Listener);
        }
        std::fs::remove_file(socket)?;
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let server =
        tiny_http::Server::from_listener(listener, None).map_err(|_| AuthError::Listener)?;
    let api = LocalApi::new(daemon)?;
    for request in server.incoming_requests() {
        respond(request, &api, policy_file);
    }
    Ok(())
}

fn respond(request: tiny_http::Request, api: &LocalApi, policy_file: &Path) {
    let mut peers = request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("X-Skillet-Peer"));
    let peer = peers.next().map(|header| header.value.as_str());
    let user = if request.method() == &tiny_http::Method::Get
        && request.url() == "/auth"
        && peers.next().is_none()
    {
        std::fs::read(policy_file)
            .ok()
            .filter(|bytes| bytes.len() <= 65_536)
            .and_then(|bytes| serde_json::from_slice::<Policy>(&bytes).ok())
            .and_then(|policy| {
                // Deserialization must not bypass validation.
                let policy = Policy::new(policy.users.into_iter().collect()).ok()?;
                authorize_peer(&policy, peer?, api)
            })
    } else {
        None
    };
    let mut response = tiny_http::Response::from_string(if user.is_some() {
        ""
    } else {
        "Access denied by Skillet identity policy"
    })
    .with_status_code(if user.is_some() { 204 } else { 403 });
    if let Some(user) = user {
        if let Ok(header) = tiny_http::Header::from_bytes("X-Skillet-User", user) {
            response.add_header(header);
        } else {
            response = tiny_http::Response::from_string("Access denied").with_status_code(403);
        }
    }
    // A client disappearing must not take down authorization for other requests.
    let _ = request.respond(response);
}

pub fn apply<S, F>(system: &S, files: &F, policy: &Policy) -> Result<(), super::CaddyError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/skillet"), Some(0o755), &root)?;
    files.ensure_directory(Path::new("/var/lib/skillet/tailnet-ui"), Some(0o700), &root)?;
    apply_socket_policy(system, files, &root)?;
    let payload = serde_json::to_vec(policy)?;
    let policy_changed = files.ensure_file(Path::new(POLICY_FILE), &payload, Some(0o600), &root)?;
    let unit = "[Unit]\nDescription=Skillet tailnet UI authorization\nRequires=skillet-tailnet-ui-policy.service\nAfter=skillet-tailnet-ui-policy.service tailscale.service\n\n[Service]\nExecStart=/var/usrlocal/bin/skillet tailnet-ui serve\nRuntimeDirectory=skillet-tailnet-ui\nRuntimeDirectoryMode=0700\nRuntimeDirectoryPreserve=yes\nRestart=on-failure\nUMask=0077\nSELinuxContext=system_u:system_r:skillet_tailnet_ui_t:s0\nNoNewPrivileges=yes\nProtectSystem=strict\nProtectHome=yes\nPrivateTmp=yes\nReadWritePaths=/run/skillet-tailnet-ui\nRestrictAddressFamilies=AF_UNIX\n\n[Install]\nWantedBy=multi-user.target\n";
    let changed = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-tailnet-ui.service"),
        unit.as_bytes(),
        Some(0o644),
        &root,
    )?;
    files.ensure_directory(Path::new("/var/lib/skillet/tailnet-ui"), Some(0o700), &root)?;
    system.service_enable("skillet-tailnet-ui.service")?;
    // The applied revision must not disclose allowed identities in a recording.
    let mut revision = Sha256::new();
    revision.update(&payload);
    revision.update(unit);
    revision.update(SOCKET_POLICY);
    activate(
        system,
        files,
        &ActivationRequest {
            service: "skillet-tailnet-ui.service".into(),
            state_path: "/var/lib/skillet/tailnet-ui/applied".into(),
            revision: revision.finalize().to_vec(),
            definition_changed: changed || policy_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

const SOCKET_POLICY: &str = include_str!("auth/skillet-tailnet-ui.cil");
const SOCKET_POLICY_UNIT: &str = include_str!("auth/skillet-tailnet-ui-policy.service");

fn apply_socket_policy<S, F>(
    system: &S,
    files: &F,
    root: &Ownership,
) -> Result<(), super::CaddyError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    files.ensure_directory(Path::new("/etc/skillet/selinux"), Some(0o755), root)?;
    let policy_changed = files.ensure_file(
        Path::new("/etc/skillet/selinux/skillet-tailnet-ui.cil"),
        SOCKET_POLICY.as_bytes(),
        Some(0o644),
        root,
    )?;
    let unit_changed = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-tailnet-ui-policy.service"),
        SOCKET_POLICY_UNIT.as_bytes(),
        Some(0o644),
        root,
    )?;
    let mut revision = Sha256::new();
    revision.update(SOCKET_POLICY);
    revision.update(SOCKET_POLICY_UNIT);
    activate(
        system,
        files,
        &ActivationRequest {
            service: "skillet-tailnet-ui-policy.service".into(),
            state_path: "/var/lib/skillet/tailnet-ui/policy.applied".into(),
            revision: revision.finalize().to_vec(),
            definition_changed: policy_changed || unit_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::OneShot,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "auth/tests.rs"]
mod tests;
