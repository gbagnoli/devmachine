use super::*;

#[test]
fn vm_ssh_accepts_repeated_loopback_forwards() {
    let parsed = Args::try_parse_from([
        "skillet",
        "test",
        "vm",
        "ssh",
        "clamps",
        "restore",
        "--forward",
        "18443:8443",
        "--forward",
        "18080:8080",
    ])
    .unwrap();
    let Commands::Test {
        command: TestCommands::Vm {
            command: VmCommands::Ssh(args),
        },
    } = parsed.command
    else {
        panic!("expected VM SSH");
    };
    assert_eq!(args.target.hostname, "clamps");
    assert_eq!(args.target.instance, "restore");
    assert_eq!(args.forward.len(), 2);
}

#[test]
fn vm_ssh_rejects_nonlocal_or_zero_port_forwards() {
    for value in [
        "0:8443",
        "18443:0",
        "0.0.0.0:18443:8443",
        "18443:remote:8443",
    ] {
        assert!(Args::try_parse_from([
            "skillet",
            "test",
            "vm",
            "ssh",
            "clamps",
            "restore",
            "--forward",
            value
        ])
        .is_err());
    }
}
