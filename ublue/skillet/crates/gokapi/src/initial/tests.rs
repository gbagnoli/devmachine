use super::*;
use std::cell::RefCell;
use std::os::unix::fs::MetadataExt;
struct Fake {
    requests: RefCell<Vec<String>>,
}
impl Api for Fake {
    fn request(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        _: Option<&str>,
    ) -> Result<(u16, Value), Error> {
        self.requests.borrow_mut().push(path.into());
        if path == "/auth/token" {
            assert!(headers.contains(&("permission", "PERM_MANAGE_USERS")));
            return Ok((200, json!({"key":"fixture-only"})));
        }
        assert!(headers.contains(&("apikey", "fixture-only")));
        assert!(headers.contains(&("username", "member@example.invalid")));
        Ok((409, Value::Null))
    }
}
#[test]
fn existing_users_are_preserved_without_ranking_or_permission_mutations() {
    let api = Fake {
        requests: RefCell::new(vec![]),
    };
    let settings = Settings {
        public_url: "https://send.example.invalid/".into(),
        admin: "admin@example.invalid".into(),
        users: vec![
            "admin@example.invalid".into(),
            "member@example.invalid".into(),
        ],
    };
    accounts(&api, &settings).unwrap();
    accounts(&api, &settings).unwrap();
    assert_eq!(
        api.requests
            .borrow()
            .iter()
            .filter(|p| p.as_str() == "/api/user/create")
            .count(),
        2
    );
    let body = setup_body(&settings).unwrap();
    let values: Vec<Value> = serde_json::from_str(&body).unwrap();
    for (name, value) in [
        ("authentication_sel", "2"),
        ("auth_header_only_registered_users", "true"),
        ("sqlite_location", "/app/config/gokapi.sqlite"),
    ] {
        assert!(values
            .iter()
            .any(|v| v["name"] == name && v["value"] == value));
    }
}

#[test]
#[ignore = "requires Podman and the pinned Gokapi image; no vault or provider access"]
fn real_gokapi_bootstrap_accounts_and_paired_restore_preserve_native_state() {
    let root = tempfile::tempdir().unwrap();
    verify_network_discovery(root.path());
    let state = root.path().join("state");
    fs::create_dir(&state).unwrap();
    for leaf in ["config", "data"] {
        fs::create_dir(state.join(leaf)).unwrap();
    }
    let owner = fs::metadata(&state).unwrap();
    let (uid, gid) = (owner.uid(), owner.gid());
    let state_path = state.to_string_lossy().into_owned();
    let backups = root.path().join("backups");
    let config = Config {
        state_path: Box::leak(state_path.into_boxed_str()),
        backup_path: Box::leak(backups.to_string_lossy().into_owned().into_boxed_str()),
        backup_subvolume: "/unused",
        owner: "unused",
        group: "unused",
    };
    let settings = Settings {
        public_url: "https://send.example.invalid/".into(),
        admin: "admin@example.invalid".into(),
        users: vec![
            "admin@example.invalid".into(),
            "member@example.invalid".into(),
        ],
    };
    // A killed first-run writer owns only disposable, bound staging state.
    fs::write(
        state.join(".skillet-bootstrap-binding.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    let partial = state.join(".skillet-bootstrap-state");
    fs::create_dir(&partial).unwrap();
    fs::write(partial.join("config.json"), b"incomplete wizard write").unwrap();
    bootstrap_new(&state, &settings, uid, gid).unwrap();
    assert!(!state.join(".skillet-bootstrap-binding.json").exists());
    assert!(state.join(".skillet-initial.json").exists());
    let container = Container::start(Path::new(config.state_path), uid, gid).unwrap();
    wait_api(
        &container,
        "/admin",
        &[("X-Skillet-User", settings.admin.as_str())],
    );
    accounts(&container, &settings).unwrap();
    verify_native_accounts(&container);
    container.stop().unwrap();
    let original = fs::read(state.join("config/config.json")).unwrap();
    validate_native(&original, &settings).unwrap();
    fs::write(state.join("data/fixture-upload"), b"fixture file bytes").unwrap();
    let saved = crate::backup::create_stopped(config, uid, gid).unwrap();
    fs::write(state.join("data/fixture-upload"), b"changed after backup").unwrap();
    crate::backup::replace_state(config, &saved, uid, gid).unwrap();
    assert_eq!(
        fs::read(state.join("config/config.json")).unwrap(),
        original
    );
    assert_eq!(
        fs::read(state.join("data/fixture-upload")).unwrap(),
        b"fixture file bytes"
    );
    let restored = Container::start(Path::new(config.state_path), uid, gid).unwrap();
    wait_api(
        &restored,
        "/admin",
        &[("X-Skillet-User", "member@example.invalid")],
    );
    accounts(&restored, &settings).unwrap();
    restored.stop().unwrap();
}
fn wait_api(api: &impl Api, path: &str, headers: &[(&str, &str)]) {
    for _ in 0..90 {
        if api
            .request(path, headers, None)
            .is_ok_and(|(status, _)| status == 200)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("native Gokapi readiness timed out");
}

fn verify_native_accounts(container: &impl Api) {
    assert_eq!(
        container
            .request(
                "/admin",
                &[("X-Skillet-User", "member@example.invalid")],
                None
            )
            .unwrap()
            .0,
        200
    );
    assert_eq!(
        container
            .request(
                "/auth/token",
                &[
                    ("X-Skillet-User", "unknown@example.invalid"),
                    ("permission", "PERM_VIEW")
                ],
                None
            )
            .unwrap()
            .0,
        401
    );
    assert_ne!(
        container
            .request(
                "/auth/token",
                &[
                    ("X-Skillet-User", "member@example.invalid"),
                    ("permission", "PERM_MANAGE_USERS")
                ],
                None
            )
            .unwrap()
            .0,
        200
    );
}

#[test]
fn interrupted_directory_publication_finishes_without_rewriting_native_identity() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path();
    let settings = Settings {
        public_url: "https://send.example.invalid/".into(),
        admin: "admin@example.invalid".into(),
        users: vec!["admin@example.invalid".into()],
    };
    fs::create_dir(state.join("config")).unwrap();
    fs::create_dir(state.join("data")).unwrap();
    let native=serde_json::to_vec(&json!({"Authentication":{"Method":2,"HeaderKey":"X-Skillet-User","OnlyRegisteredUsers":true,"Username":settings.admin},"ServerUrl":settings.public_url,"DatabaseUrl":"sqlite:///app/config/gokapi.sqlite","DataDir":"/app/data","UseSsl":false})).unwrap();
    fs::write(state.join("config/config.json"), &native).unwrap();
    fs::write(state.join("config/gokapi.sqlite"), b"fixture database").unwrap();
    let staging = state.join(".skillet-bootstrap-state");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(staging.join("data")).unwrap();
    fs::write(staging.join("data/fixture"), b"complete prepared data").unwrap();
    fs::write(
        state.join(".skillet-bootstrap-binding.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    publish_initial(state, &settings).unwrap();
    assert_eq!(fs::read(state.join("config/config.json")).unwrap(), native);
    assert_eq!(
        fs::read(state.join("data/fixture")).unwrap(),
        b"complete prepared data"
    );
    assert!(!state.join(".skillet-bootstrap-binding.json").exists());
    assert!(state.join(".skillet-initial.json").is_file());
}

#[test]
fn forwarded_ip_trust_uses_only_the_observed_dedicated_isolated_network() {
    let network = json!([{"name":"fixture-gokapi","driver":"bridge","internal":false,"options":{"isolate":"strict"},"subnets":[{"subnet":"10.89.4.0/24"},{"subnet":"fd00:1234::/64"}]}]);
    assert_eq!(
        proxy_subnets("fixture-gokapi", &serde_json::to_vec(&network).unwrap()).unwrap(),
        ["10.89.4.0/24", "fd00:1234::/64"]
    );
    assert!(proxy_subnets("another-network", &serde_json::to_vec(&network).unwrap()).is_err());
    let mut broad = network;
    broad[0]["subnets"][0]["subnet"] = json!("0.0.0.0/0");
    assert!(proxy_subnets("fixture-gokapi", &serde_json::to_vec(&broad).unwrap()).is_err());
}

fn verify_network_discovery(root: &Path) {
    struct Network(String);
    impl Drop for Network {
        fn drop(&mut self) {
            let _ = run(&["network", "rm", &self.0], None);
        }
    }
    let name = format!(
        "skillet-gokapi-policy-{}",
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .replace('.', "-")
    );
    let output = run(
        &[
            "network",
            "create",
            "--ipv6",
            "--opt",
            "isolate=strict",
            &name,
        ],
        None,
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let network = Network(name);
    let inspected = run(&["network", "inspect", &network.0], None).unwrap();
    assert!(inspected.status.success());
    assert_eq!(
        proxy_subnets(&network.0, &inspected.stdout).unwrap().len(),
        2
    );
}
