//! Native `SELinux` adapter for volatile SMTP configuration files.
use crate::CliCommonError;
use skillet_core::files::FileReadResource;
use std::path::Path;

pub(super) fn label_credentials(
    files: &impl FileReadResource,
    execute: &impl Fn(&str, &[&str]) -> Result<(), CliCommonError>,
) -> Result<(), CliCommonError> {
    // SELinux-disabled containers have no mounted policy to label against.
    // Permissive hosts still need correct labels before enforcement is enabled.
    if files
        .read_file(Path::new("/sys/fs/selinux/enforce"))
        .map_err(|e| CliCommonError::Config(e.to_string()))?
        .is_none()
    {
        return Ok(());
    }
    execute("/usr/sbin/restorecon", &["-F", "/etc/postfix/main.cf"])?;
    execute(
        "/usr/bin/chcon",
        &[
            "-R",
            "--reference=/etc/postfix/main.cf",
            "/run/postfix/skillet",
        ],
    )
}

#[cfg(test)]
#[path = "smtp_runtime/tests.rs"]
mod tests;
