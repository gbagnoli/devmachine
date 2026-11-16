use super::Args;
use clap::Parser;
#[test]
fn image_entrypoint_accepts_application_reconciliation_and_backup_commands() {
    for args in [
        vec![
            "skillet",
            "media",
            "reconcile",
            "--host-file",
            "/etc/skillet/host",
        ],
        vec![
            "skillet",
            "postgres",
            "backup",
            "--host-file",
            "/etc/skillet/host",
            "--instance",
            "joplin",
        ],
        vec![
            "skillet",
            "postgres",
            "restore",
            "--instance",
            "joplin",
            "--archive",
            "/var/tmp/fixture.dump",
        ],
        vec![
            "skillet",
            "airtrail",
            "backup",
            "--host-file",
            "/etc/skillet/host",
        ],
        vec![
            "skillet",
            "airtrail",
            "restore",
            "--archive",
            "/var/tmp/fixture.tar",
        ],
    ] {
        Args::try_parse_from(&args).unwrap_or_else(|error| panic!("{args:?}: {error}"));
    }
}
