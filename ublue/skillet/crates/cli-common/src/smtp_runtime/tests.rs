use super::*;
use skillet_core::{
    files::{FileMutationResource, Ownership},
    test_utils::MockFiles,
};
use std::cell::RefCell;

#[test]
fn disabled_selinux_skips_label_commands() {
    label_credentials(&MockFiles::new(), &|_, _| {
        panic!("no SELinux commands expected")
    })
    .unwrap();
}

#[test]
fn enabled_and_permissive_selinux_restore_reference_then_label_credentials() {
    for enforcement in [b"0".as_slice(), b"1".as_slice()] {
        let files = MockFiles::new();
        files
            .ensure_file(
                Path::new("/sys/fs/selinux/enforce"),
                enforcement,
                None,
                &Ownership::default(),
            )
            .unwrap();
        let calls = RefCell::new(Vec::new());
        label_credentials(&files, &|program, args| {
            calls.borrow_mut().push((
                program.to_string(),
                args.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            *calls.borrow(),
            vec![
                (
                    "/usr/sbin/restorecon".into(),
                    vec!["-F".into(), "/etc/postfix/main.cf".into()]
                ),
                (
                    "/usr/bin/chcon".into(),
                    vec![
                        "-R".into(),
                        "--reference=/etc/postfix/main.cf".into(),
                        "/run/postfix/skillet".into()
                    ]
                ),
            ]
        );
        assert!(
            label_credentials(&files, &|_, _| Err(CliCommonError::Config(
                "label failure".into()
            )))
            .is_err()
        );
    }
}
