use super::*;

#[test]
fn stdin_and_large_output_are_memory_backed_and_do_not_deadlock() {
    let input = vec![b'x'; 1024 * 1024];
    let output = capture_with_input(
        Command::new("/usr/bin/cat"),
        Duration::from_secs(5),
        Some(&input),
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, input);
    assert!(output.stderr.is_empty());
}

#[test]
fn blocked_stdin_writer_and_noninteractive_child_are_bounded() {
    let mut command = Command::new("/usr/bin/sleep");
    command.arg("5");
    let input = vec![b'x'; 1024 * 1024];
    let start = Instant::now();
    assert!(matches!(
        capture_with_input(command, Duration::from_millis(30), Some(&input)),
        Err(Error::Timeout)
    ));
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn nonzero_exit_is_preserved_when_the_child_does_not_consume_stdin() {
    let output = capture_with_input(
        Command::new("/usr/bin/false"),
        Duration::from_secs(2),
        Some(&vec![b'x'; 1024 * 1024]),
    )
    .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}
