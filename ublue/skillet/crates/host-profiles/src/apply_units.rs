//! Shared apply units are Ignition-seeded and then converged by the guest.
use crate::ApplyError;
use skillet_core::{
    files::{FileResource, Ownership},
    system::SystemResource,
};
use std::path::Path;

const UNITS: [(&str, &str); 5] = [
    (
        "skillet-apply.service",
        "[Unit]\nDescription=Apply Skillet base configuration\nRequires=ucore-bootstrap.service\nAfter=ucore-bootstrap.service\nConditionPathExists=/run/ucore-bootstrap-ready\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase base\n\n[Install]\nWantedBy=multi-user.target\n",
    ),
    (
        "skillet-caddy-apply.service",
        "[Unit]\nDescription=Apply Skillet Caddy private UI configuration\nRequires=skillet-apply.service\nAfter=skillet-apply.service\nAssertPathIsMountPoint=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/caddy_sites.cred\nConditionPathExists=/etc/credstore.encrypted/skillet/cloudflare_acme_token.cred\n\n[Service]\nType=oneshot\nLoadCredentialEncrypted=caddy_sites:/etc/credstore.encrypted/skillet/caddy_sites.cred\nLoadCredentialEncrypted=cloudflare_acme_token:/etc/credstore.encrypted/skillet/cloudflare_acme_token.cred\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase caddy\n\n[Install]\nWantedBy=multi-user.target\n",
    ),
    (
        "skillet-ddns-apply.service",
        "[Unit]\nDescription=Apply Skillet optional Cloudflare DDNS configuration\nRequires=skillet-apply.service skillet-data-prepare.service\nAfter=skillet-apply.service skillet-data-prepare.service\nAssertPathIsMountPoint=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/cloudflare_ddns_config.cred\n\n[Service]\nType=oneshot\nLoadCredentialEncrypted=cloudflare_ddns_config:/etc/credstore.encrypted/skillet/cloudflare_ddns_config.cred\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase ddns\n\n[Install]\nWantedBy=multi-user.target\n",
    ),
    (
        "skillet-datadog-apply.service",
        "[Unit]\nDescription=Apply Skillet optional Datadog monitoring configuration\nRequires=skillet-apply.service skillet-data-prepare.service\nAfter=skillet-apply.service skillet-data-prepare.service\nAssertPathIsMountPoint=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/datadog_config.cred\n\n[Service]\nType=oneshot\nLoadCredentialEncrypted=datadog_config:/etc/credstore.encrypted/skillet/datadog_config.cred\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase datadog\n\n[Install]\nWantedBy=multi-user.target\n",
    ),
    (
        "skillet-smtp-apply.service",
        "[Unit]\nDescription=Apply shared fleet SMTP configuration\nRequires=skillet-apply.service\nAfter=skillet-apply.service\nConditionPathExists=/etc/credstore.encrypted/skillet/smtp_config.cred\n\n[Service]\nType=oneshot\nLoadCredentialEncrypted=smtp_config:/etc/credstore.encrypted/skillet/smtp_config.cred\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase smtp\n\n[Install]\nWantedBy=multi-user.target\n",
    ),
];

const FULL_APPLY_UNIT: &str = "[Unit]\nDescription=Apply declared Skillet application configuration\nRequires=skillet-apply.service skillet-data-prepare.service\nAfter=skillet-apply.service skillet-data-prepare.service\nAssertPathIsMountPoint=/var/lib/data\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/skillet apply --host-file /etc/skillet/host --phase full\n\n[Install]\nWantedBy=multi-user.target\n";

pub(super) fn install(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    let root = Ownership::named(Some("root"), Some("root"));
    ensure_units(system, files, &UNITS, &root)?;
    Ok(())
}

pub(super) fn install_full(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    let root = Ownership::named(Some("root"), Some("root"));
    let full = [("skillet-full-apply.service", FULL_APPLY_UNIT)];
    ensure_units(system, files, &full, &root)?;
    Ok(())
}

fn ensure_units(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    units: &[(&str, &str)],
    root: &Ownership,
) -> Result<(), ApplyError> {
    let mut changed = false;
    for (name, contents) in units {
        changed |= files.ensure_file(
            &Path::new("/etc/systemd/system").join(name),
            contents.as_bytes(),
            Some(0o644),
            root,
        )?;
    }
    if changed {
        system.daemon_reload()?;
    }
    Ok(())
}
