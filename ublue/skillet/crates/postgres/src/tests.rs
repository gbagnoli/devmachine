use super::{Config, DEFAULT_IMAGE};

#[test]
fn host_service_network_names_accept_hyphens() {
    let config = Config {
        instance: "joplin",
        database: "joplin",
        username: "joplin",
        data_path: "/var/lib/data/joplin/postgres",
        network_name: "donbot-joplin",
        image: DEFAULT_IMAGE,
        data_container_path: "/var/lib/postgresql",
    };

    assert!(config.validate().is_ok());
}
