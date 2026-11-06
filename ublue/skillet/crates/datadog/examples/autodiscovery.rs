//! A passive fixture for checking labels with the real Quadlet generator.
fn main() {
    println!("[Container]\nImage=registry.fedoraproject.org/fedora:latest");
    println!(
        "{}",
        skillet_datadog::http("syncthing", "http://%%host%%:8384/rest/noauth/health")
    );
}
