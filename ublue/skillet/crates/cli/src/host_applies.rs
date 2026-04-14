use skillet_core::{credentials::CredentialManager, files::FileResource, system::SystemResource};
use skillet_pihole;
use skillet_podman::{QuadletSecret, SecretTarget};

mod user_lookup {
    use skillet_core::system::SystemError;
    use users::{get_group_by_name, get_user_by_name};

    /// Look up UID for a username, returns None if user doesn't exist
    pub fn lookup_uid(username: &str) -> Result<Option<u32>, SystemError> {
        match get_user_by_name(username) {
            Some(user) => Ok(Some(user.uid())),
            None => Ok(None),
        }
    }

    /// Look up GID for a group name, returns None if group doesn't exist
    pub fn lookup_gid(groupname: &str) -> Result<Option<u32>, SystemError> {
        match get_group_by_name(groupname) {
            Some(group) => Ok(Some(group.gid())),
            None => Ok(None),
        }
    }
}

/// Apply configuration for a specific host
pub fn apply_host(
    hostname: &str,
    system: &(impl SystemResource + ?Sized),
    files: &(impl FileResource + ?Sized),
    credentials: &CredentialManager,
) -> Result<(), String> {
    match hostname {
        "beezelbot" => {
            skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;
        }
        "clamps" => {
            skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;

            // 1. Ingest secret from systemd
            let secret_payload = credentials
                .read_secret("test_secret")
                .map_err(|e| e.to_string())?;

            // 2. Provision to Podman
            system
                .ensure_podman_secret("pihole_web_password", &secret_payload)
                .map_err(|e| e.to_string())?;

            // Look up pihole user and group IDs
            let (pihole_uid_opt, pihole_gid_opt) = match (
                user_lookup::lookup_uid("pihole"),
                user_lookup::lookup_gid("pihole"),
            ) {
                (Ok(uid), Ok(gid)) => (uid, gid),
                _ => {
                    // If user/group doesn't exist yet, we'll create them with dynamic IDs
                    // For now, use placeholders that will be replaced during ensure_user/group
                    (None, None)
                }
            };

            // 3. Apply pihole with the secret
            let secrets = vec![QuadletSecret {
                secret_name: "pihole_web_password".to_string(),
                target: SecretTarget::File {
                    target_path: "/etc/pihole/webpassword".to_string(),
                    mode: Some("0400".to_string()),
                    uid: pihole_uid_opt,
                    gid: pihole_gid_opt,
                },
            }];

            skillet_pihole::apply(
                system,
                files,
                skillet_pihole::PiholeUser {
                    uid: pihole_uid_opt,
                    gid: pihole_gid_opt,
                    name: "pihole".to_string(),
                    group_name: "pihole".to_string(),
                },
                secrets,
            )
            .map_err(|e: skillet_pihole::PiholeError| e.to_string())?;
        }
        _ => {
            // Default fallback: just hardening
            skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
