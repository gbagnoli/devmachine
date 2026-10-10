use super::super::{Args, Commands as RootCommands};
use super::*;
use clap::Parser as _;

#[test]
fn physical_command_requires_explicit_inputs_and_defaults_to_plain_root() {
    assert!(Args::try_parse_from(["skillet", "install", "prepare", "clamps"]).is_err());
    let args = Args::try_parse_from([
        "skillet",
        "install",
        "prepare",
        "clamps",
        "--disk",
        "/dev/disk/by-id/ata-example",
        "--iso",
        "/private/live.iso",
        "--output",
        "/private/bundle",
        "--ssh-public-key",
        "/private/key.pub",
        "--password-hash-file",
        "/private/password.hash",
    ])
    .unwrap();
    let RootCommands::Install {
        command: Commands::Prepare(request),
    } = args.command
    else {
        panic!("wrong command dispatch");
    };
    assert_eq!(request.host, "clamps");
    assert!(matches!(request.root_profile, RootProfileName::Unencrypted));
}

#[test]
fn physical_and_vm_help_remain_available() {
    for args in [
        vec!["skillet", "install", "prepare", "--help"],
        vec!["skillet", "test", "vm", "create", "--help"],
    ] {
        assert_eq!(
            Args::try_parse_from(args).unwrap_err().kind(),
            clap::error::ErrorKind::DisplayHelp
        );
    }
}
