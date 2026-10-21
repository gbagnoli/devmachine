//! Keep bootstrap and retained host units aligned with selected full credentials.
use crate::{ApplyError, HostApplyPhase, HostProfile};
use skillet_core::{
    files::{FileResource, Ownership},
    system::SystemResource,
};
use std::{fmt::Write as _, path::Path};

pub(super) fn render(profile: &HostProfile) -> String {
    let credentials = crate::credential_names_for_phase(profile, HostApplyPhase::Full);
    let mut unit = String::from("[Unit]\nConditionPathExists=\n");
    for name in &credentials {
        let _ = writeln!(
            unit,
            "ConditionPathExists=/etc/credstore.encrypted/skillet/{name}.cred"
        );
    }
    unit.push_str("\n[Service]\nLoadCredentialEncrypted=\n");
    for name in &credentials {
        let _ = writeln!(
            unit,
            "LoadCredentialEncrypted={name}:/etc/credstore.encrypted/skillet/{name}.cred"
        );
    }
    unit
}

pub(super) fn install(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    profile: &HostProfile,
) -> Result<(), ApplyError> {
    if !profile.requires_full_apply_credentials() {
        return Ok(());
    }
    let directory = Path::new("/etc/systemd/system/skillet-full-apply.service.d");
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(directory, Some(0o755), &root)?;
    let unit = render(profile);
    let changed = files.ensure_file(
        &directory.join("credentials.conf"),
        unit.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let state = directory.join("credentials.applied");
    if changed || files.read_file(&state)?.as_deref() != Some(unit.as_bytes()) {
        system.daemon_reload()?;
        files.ensure_file(&state, unit.as_bytes(), Some(0o644), &root)?;
    }
    install_joplin(system, files, profile)?;
    Ok(())
}

fn install_joplin(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    profile: &HostProfile,
) -> Result<(), ApplyError> {
    if !profile.supports_service("joplin") {
        return Ok(());
    }
    let path = Path::new("/etc/systemd/system/skillet-joplin-apply.service");
    let root = Ownership::named(Some("root"), Some("root"));
    let unit = format!(
        "[Unit]\nDescription=Apply the declared Joplin service\nAfter=skillet-apply.service\nRequires=skillet-apply.service\nRequiresMountsFor=/var/lib/data\nConditionPathExists=/etc/credstore.encrypted/skillet/{}.cred\nConditionPathExists=/etc/credstore.encrypted/skillet/{}.cred\n[Service]\nType=oneshot\nLoadCredentialEncrypted={}:/etc/credstore.encrypted/skillet/{}.cred\nLoadCredentialEncrypted={}:/etc/credstore.encrypted/skillet/{}.cred\nExecStart=/var/usrlocal/bin/skillet-{} apply --phase joplin\n[Install]\nWantedBy=multi-user.target\n",
        crate::CADDY_SITES_CREDENTIAL,
        skillet_joplin::DATABASE_PASSWORD_CREDENTIAL,
        crate::CADDY_SITES_CREDENTIAL,
        crate::CADDY_SITES_CREDENTIAL,
        skillet_joplin::DATABASE_PASSWORD_CREDENTIAL,
        skillet_joplin::DATABASE_PASSWORD_CREDENTIAL,
        profile.id.as_str(),
    );
    let changed = files.ensure_file(path, unit.as_bytes(), Some(0o644), &root)?;
    if changed {
        system.daemon_reload()?;
    }
    system.service_enable("skillet-joplin-apply.service")?;
    Ok(())
}

#[cfg(test)]
#[path = "application_unit/tests.rs"]
mod tests;
