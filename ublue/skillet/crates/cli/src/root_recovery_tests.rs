use super::*;
#[test]
fn production_recovery_requires_explicit_verified_ssh_target() {
    assert!(Args::try_parse_from(["skillet", "secrets", "root-recovery-key", "clamps"]).is_err());
    let parsed = Args::try_parse_from([
        "skillet",
        "secrets",
        "root-recovery-key",
        "clamps",
        "--target",
        "admin@host",
        "--identity",
        "/tmp/key",
        "--known-hosts",
        "/tmp/known",
        "--database",
        "/tmp/vault",
    ])
    .unwrap();
    let Commands::Secret {
        command: SecretCommands::RootRecoveryKey(args),
    } = parsed.command
    else {
        panic!("wrong command");
    };
    assert_eq!(args.port, 22);
    assert_eq!(args.vault.database, Some(PathBuf::from("/tmp/vault")));
}
#[test]
fn disposable_recovery_keeps_instance_distinct_and_loads_target_from_owner() {
    let parsed = Args::try_parse_from([
        "skillet",
        "test",
        "vm",
        "recovery-key",
        "clamps",
        "encrypted",
        "--database",
        "/tmp/vault",
    ])
    .unwrap();
    let Commands::Test {
        command: TestCommands::Vm {
            command: VmCommands::RecoveryKey(args),
        },
    } = parsed.command
    else {
        panic!("wrong command");
    };
    assert_eq!(args.target.hostname, "clamps");
    assert_eq!(args.target.instance, "encrypted");
    assert!(Args::try_parse_from([
        "skillet",
        "test",
        "vm",
        "recovery-key",
        "clamps",
        "encrypted",
        "--target",
        "admin@arbitrary"
    ])
    .is_err());
}
