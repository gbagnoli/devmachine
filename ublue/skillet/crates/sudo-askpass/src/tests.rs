use super::*;
use std::os::unix::net::UnixListener;

#[test]
fn forwards_only_a_valid_bounded_password_response() {
    let directory =
        std::env::temp_dir().join(format!("skillet-askpass-unit-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let listener = UnixListener::bind(directory.join("askpass.sock")).unwrap();
    let server = std::thread::spawn(move || {
        for response in [
            b"synthetic password\n".as_slice(),
            b"bad\nextra\n".as_slice(),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 24];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"skillet-sudo-askpass-v1\n");
            stream.write_all(response).unwrap();
        }
    });
    let mut result = Vec::new();
    run(&directory.join("helper"), &mut result).unwrap();
    assert_eq!(result, b"synthetic password\n");
    result.clear();
    assert!(run(&directory.join("helper"), &mut result).is_err());
    assert_eq!(result, []);
    server.join().unwrap();
    std::fs::remove_file(directory.join("askpass.sock")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
