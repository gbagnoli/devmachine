use super::*;
use std::io::{Read, Write};
#[test]
fn relay_is_bidirectional_and_closes_cleanly() {
    let server = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = server.local_addr().unwrap().port();
    let (mut client, relay_socket) = UnixStream::pair().unwrap();
    let upstream = std::thread::spawn(move || {
        let (mut connection, _) = server.accept().unwrap();
        let mut bytes = [0; 4];
        connection.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ping");
        connection.write_all(b"pong").unwrap();
    });
    let relay = std::thread::spawn(move || super::relay(relay_socket, port).unwrap());
    client.write_all(b"ping").unwrap();
    let mut bytes = [0; 4];
    client.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"pong");
    drop(client);
    upstream.join().unwrap();
    relay.join().unwrap();
}
