use clap::Parser as _;
#[test]
fn auth_lock_is_independent_of_host_vault_and_ssh_arguments() {
    let args = crate::Args::try_parse_from(["skillet", "auth", "lock"]).unwrap();
    assert!(matches!(
        args.command,
        crate::Commands::Auth {
            command: super::Commands::Lock
        }
    ));
}
