use super::*;
use std::{io::Write as _, net::TcpListener};

#[test]
fn api_uses_versioned_paths_and_header_keys_and_suppresses_response_secrets() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0; 4096];
        let count = stream.read(&mut bytes).unwrap();
        let request = String::from_utf8_lossy(&bytes[..count]);
        assert!(request.starts_with("GET /api/v3/system/status "));
        assert!(request
            .to_lowercase()
            .contains("x-api-key: 00000000000000000000000000000001"));
        stream
            .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 18\r\n\r\nsecret-do-not-echo")
            .unwrap();
    });
    let credentials = crate::fixtures::credentials();
    let api = HttpApi::new(BTreeMap::from([(App::Sonarr, endpoint)]), &credentials).unwrap();
    let error = api
        .request(App::Sonarr, Method::Get, "system/status", None)
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 400"));
    assert!(!error.to_string().contains("secret-do-not-echo"));
    server.join().unwrap();
}
