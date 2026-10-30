use super::*;
use skillet_core::{files::LocalFileResource, test_utils::MockFiles};
use std::cell::Cell;

const XML: &[u8] = br#"<configuration version="52"><device id="LOCAL" name="old"><address>dynamic</address></device><gui enabled="true"><apikey>generated-key</apikey><insecureAdminAccess>false</insecureAdminAccess></gui><options><urAccepted>0</urAccepted></options></configuration>"#;
fn settings() -> Settings {
    Settings {
        instance: crate::Instance::Shared,
        host: "clamps".into(),
        environment: "prod".into(),
        runtime_hostname: "clamps".into(),
        device_name: "clamps.example.invalid".into(),
    }
}
struct Fake<'a> {
    files: &'a MockFiles,
    calls: Cell<u32>,
}
impl Generator for Fake<'_> {
    fn generate(&self, stage: &Path) -> Result<(), Error> {
        self.calls.set(self.calls.get() + 1);
        for (name, bytes) in [
            ("cert.pem", &b"CERT"[..]),
            ("key.pem", &b"KEY"[..]),
            ("config.xml", XML),
        ] {
            self.files.create_file_if_absent(
                &stage.join(name),
                bytes,
                Some(0o600),
                &Ownership::default(),
            )?;
        }
        Ok(())
    }
}
#[test]
fn complete_seed_and_existing_xml_preservation() {
    let files = MockFiles::new();
    let generator = Fake {
        files: &files,
        calls: Cell::new(0),
    };
    let root = Path::new("/fixture/config");
    assert!(initialize(
        &files,
        &generator,
        root,
        &settings(),
        true,
        &Ownership::default()
    )
    .unwrap());
    let xml = files.read_file(&root.join("config.xml")).unwrap().unwrap();
    let text = String::from_utf8(xml.clone()).unwrap();
    assert!(text.contains("name=\"clamps.example.invalid\""));
    assert!(text.contains("<insecureAdminAccess>true</insecureAdminAccess>"));
    assert!(text.contains("generated-key"));
    let mut changed = settings();
    changed.device_name = "clamps.changed.invalid".into();
    assert!(!initialize(
        &files,
        &generator,
        root,
        &changed,
        false,
        &Ownership::default()
    )
    .unwrap());
    assert_eq!(generator.calls.get(), 1);
    assert_eq!(
        files.read_file(&root.join("config.xml")).unwrap().unwrap(),
        xml
    );
}
#[test]
fn preserves_complete_keys_and_rejects_incomplete_pair() {
    for complete in [false, true] {
        let files = MockFiles::new();
        let root = Path::new("/fixture/config");
        files
            .ensure_file(
                &root.join("cert.pem"),
                b"OLD-CERT",
                None,
                &Ownership::default(),
            )
            .unwrap();
        if complete {
            files
                .ensure_file(
                    &root.join("key.pem"),
                    b"OLD-KEY",
                    None,
                    &Ownership::default(),
                )
                .unwrap();
        }
        let generator = Fake {
            files: &files,
            calls: Cell::new(0),
        };
        let result = initialize(
            &files,
            &generator,
            root,
            &settings(),
            true,
            &Ownership::default(),
        );
        assert_eq!(result.is_ok(), complete);
        assert_eq!(
            files.read_file(&root.join("cert.pem")).unwrap().unwrap(),
            b"OLD-CERT"
        );
        if complete {
            assert_eq!(
                files.read_file(&root.join("key.pem")).unwrap().unwrap(),
                b"OLD-KEY"
            );
        } else {
            assert_eq!(generator.calls.get(), 0);
        }
    }
}
#[test]
fn resumes_partial_publication_and_rejects_conflicting_keys() {
    for conflicting in [false, true] {
        let files = MockFiles::new();
        let root = Path::new("/fixture/config");
        let stage = root.join(".skillet-initial");
        for (path, bytes) in [
            (stage.join("settings.json"), settings().payload().unwrap()),
            (stage.join("cert.pem"), b"CERT".to_vec()),
            (stage.join("key.pem"), b"KEY".to_vec()),
            (
                root.join("cert.pem"),
                if conflicting {
                    b"FOREIGN".to_vec()
                } else {
                    b"CERT".to_vec()
                },
            ),
        ] {
            files
                .ensure_file(&path, &bytes, None, &Ownership::default())
                .unwrap();
        }
        let generator = Fake {
            files: &files,
            calls: Cell::new(0),
        };
        assert_eq!(
            initialize(
                &files,
                &generator,
                root,
                &settings(),
                true,
                &Ownership::default()
            )
            .is_ok(),
            !conflicting
        );
    }
}
#[test]
fn malformed_generated_xml_and_scope_are_rejected() {
    assert!(configure(b"<configuration/>", "a.example.invalid", true).is_err());
    let mut input = settings();
    input.runtime_hostname = "different".into();
    assert!(input.validate().is_err());
    let files = MockFiles::new();
    let root = Path::new("/fixture/config");
    files
        .ensure_file(
            &root.join(".skillet-initial/settings.json"),
            b"wrong",
            None,
            &Ownership::default(),
        )
        .unwrap();
    let generator = Fake {
        files: &files,
        calls: Cell::new(0),
    };
    assert!(matches!(
        initialize(
            &files,
            &generator,
            root,
            &settings(),
            true,
            &Ownership::default()
        ),
        Err(Error::Binding)
    ));
    assert_eq!(generator.calls.get(), 0);
}
#[test]
fn acknowledgement_follows_explicit_caller_policy() {
    assert!(
        String::from_utf8(configure(XML, "a.example.invalid", false).unwrap())
            .unwrap()
            .contains("<insecureAdminAccess>false</insecureAdminAccess>")
    );
}

struct RealGenerator;
impl Generator for RealGenerator {
    fn generate(&self, stage: &Path) -> Result<(), Error> {
        let status = std::process::Command::new("podman")
            .args([
                "run",
                "--rm",
                "--network",
                "none",
                "--hostname",
                "fixture.example.invalid",
                "--entrypoint",
                "/bin/syncthing",
                "--volume",
            ])
            .arg(format!("{}:/var/syncthing/config:Z", stage.display()))
            .args([IMAGE, "generate", "--home", "/var/syncthing/config"])
            .status()
            .map_err(|_| Error::Generator)?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Generator)
        }
    }
}
#[test]
#[ignore = "requires real Podman consumer; executed separately in CI"]
fn real_upstream_generator_preserves_identity_and_user_edits() {
    let temp = tempfile::tempdir().unwrap();
    let files = LocalFileResource;
    let root = temp.path();
    let input = settings();
    assert!(initialize(
        &files,
        &RealGenerator,
        root,
        &input,
        true,
        &Ownership::default()
    )
    .unwrap());
    let cert = std::fs::read(root.join("cert.pem")).unwrap();
    let key = std::fs::read(root.join("key.pem")).unwrap();
    let original = std::fs::read(root.join("config.xml")).unwrap();
    let first = Running::start(root);
    let initial_config = first.configuration();
    assert_eq!(initial_config["devices"][0]["name"], input.device_name);
    assert_eq!(initial_config["gui"]["insecureAdminAccess"], true);
    assert!(initial_config["gui"]["apiKey"]
        .as_str()
        .is_some_and(|key| !key.is_empty()));
    let device_id = initial_config["devices"][0]["deviceID"].clone();
    assert!(device_id.as_str().is_some_and(|id| !id.is_empty()));
    drop(first);
    let edited = String::from_utf8(original)
        .unwrap()
        .replace("clamps.example.invalid", "user-chosen-name")
        .replace("<insecureAdminAccess>true", "<insecureAdminAccess>false");
    std::fs::write(root.join("config.xml"), &edited).unwrap();
    assert!(!initialize(
        &files,
        &RealGenerator,
        root,
        &input,
        true,
        &Ownership::default()
    )
    .unwrap());
    assert_eq!(
        std::fs::read(root.join("config.xml")).unwrap(),
        edited.as_bytes()
    );
    let restarted = Running::start(root);
    let user_config = restarted.configuration();
    assert_eq!(user_config["devices"][0]["name"], "user-chosen-name");
    assert_eq!(user_config["gui"]["insecureAdminAccess"], false);
    assert_eq!(user_config["devices"][0]["deviceID"], device_id);
    drop(restarted);
    // An authorized XML-only reset must retain the exact device keypair.
    std::fs::remove_file(root.join("config.xml")).unwrap();
    assert!(initialize(
        &files,
        &RealGenerator,
        root,
        &input,
        true,
        &Ownership::default()
    )
    .unwrap());
    assert_eq!(std::fs::read(root.join("cert.pem")).unwrap(), cert);
    assert_eq!(std::fs::read(root.join("key.pem")).unwrap(), key);
    let reset = Running::start(root);
    let reset_config = reset.configuration();
    assert_eq!(reset_config["devices"][0]["name"], input.device_name);
    assert_eq!(reset_config["gui"]["insecureAdminAccess"], true);
    assert_eq!(reset_config["devices"][0]["deviceID"], device_id);
}

struct Running(String);
impl Running {
    fn start(root: &Path) -> Self {
        let output = std::process::Command::new("podman")
            .args([
                "run",
                "--detach",
                "--network",
                "none",
                "--entrypoint",
                "/bin/syncthing",
                "--volume",
            ])
            .arg(format!("{}:/var/syncthing/config:Z", root.display()))
            .args([
                IMAGE,
                "serve",
                "--home",
                "/var/syncthing/config",
                "--no-browser",
                "--no-restart",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "failed to start fixture consumer");
        let id = String::from_utf8(output.stdout).unwrap().trim().to_string();
        assert_eq!(id.len(), 64);
        Self(id)
    }
    fn configuration(&self) -> serde_json::Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let output = std::process::Command::new("podman")
                .args([
                    "exec",
                    "--env",
                    "STGUIADDRESS=",
                    &self.0,
                    "/bin/syncthing",
                    "cli",
                    "--home",
                    "/var/syncthing/config",
                    "config",
                    "dump-json",
                ])
                .output()
                .unwrap();
            if output.status.success() {
                return serde_json::from_slice(&output.stdout).unwrap();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixture API did not become ready"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "--force", &self.0])
            .output();
    }
}

struct Interrupted<'a> {
    files: &'a MockFiles,
    remaining: Cell<usize>,
}
impl FileReadResource for Interrupted<'_> {
    fn read_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FileError> {
        self.files.read_file(path)
    }
}
impl FileMutationResource for Interrupted<'_> {
    fn create_file_if_absent(
        &self,
        path: &Path,
        bytes: &[u8],
        mode: Option<u32>,
        owner: &Ownership,
    ) -> Result<bool, FileError> {
        let created = self.files.create_file_if_absent(path, bytes, mode, owner)?;
        let remaining = self.remaining.get();
        self.remaining.set(remaining.saturating_sub(1));
        if remaining == 1 {
            return Err(FileError::Io(std::io::Error::other(
                "interrupted publication",
            )));
        }
        Ok(created)
    }
    fn ensure_file(
        &self,
        path: &Path,
        bytes: &[u8],
        mode: Option<u32>,
        owner: &Ownership,
    ) -> Result<bool, FileError> {
        self.files.ensure_file(path, bytes, mode, owner)
    }
    fn ensure_directory(
        &self,
        path: &Path,
        mode: Option<u32>,
        owner: &Ownership,
    ) -> Result<bool, FileError> {
        self.files.ensure_directory(path, mode, owner)
    }
    fn delete_file(&self, path: &Path) -> Result<bool, FileError> {
        self.files.delete_file(path)
    }
}
#[test]
fn retries_every_publication_checkpoint_without_replacing_keys() {
    for checkpoint in 1..=4 {
        let files = MockFiles::new();
        let root = Path::new("/fixture/config");
        let generator = Fake {
            files: &files,
            calls: Cell::new(0),
        };
        let interrupted = Interrupted {
            files: &files,
            remaining: Cell::new(checkpoint),
        };
        assert!(initialize(
            &interrupted,
            &generator,
            root,
            &settings(),
            true,
            &Ownership::default()
        )
        .is_err());
        initialize(
            &files,
            &generator,
            root,
            &settings(),
            true,
            &Ownership::default(),
        )
        .unwrap();
        assert!(files.read_file(&root.join("config.xml")).unwrap().is_some());
        assert_eq!(
            files.read_file(&root.join("cert.pem")).unwrap().unwrap(),
            b"CERT"
        );
        assert_eq!(
            files.read_file(&root.join("key.pem")).unwrap().unwrap(),
            b"KEY"
        );
    }
}

#[test]
fn recovers_half_generated_owned_staging_without_a_live_identity() {
    let files = MockFiles::new();
    let root = Path::new("/fixture/config");
    files
        .ensure_file(
            &root.join(".skillet-initial/settings.json"),
            &settings().payload().unwrap(),
            None,
            &Ownership::default(),
        )
        .unwrap();
    files
        .ensure_file(
            &root.join(".skillet-initial/cert.pem"),
            b"PARTIAL",
            None,
            &Ownership::default(),
        )
        .unwrap();
    let generator = Fake {
        files: &files,
        calls: Cell::new(0),
    };
    assert!(initialize(
        &files,
        &generator,
        root,
        &settings(),
        true,
        &Ownership::default()
    )
    .unwrap());
    assert_eq!(
        files.read_file(&root.join("cert.pem")).unwrap().unwrap(),
        b"CERT"
    );
}
#[test]
fn default_omitted_and_empty_gui_flags_are_initialized_once() {
    for original in [
        "<insecureAdminAccess>false</insecureAdminAccess>",
        "<insecureAdminAccess/>",
    ] {
        let text = String::from_utf8(XML.to_vec())
            .unwrap()
            .replace("<insecureAdminAccess>false</insecureAdminAccess>", original);
        let output =
            String::from_utf8(configure(text.as_bytes(), "fixture.example.invalid", true).unwrap())
                .unwrap();
        assert_eq!(
            output
                .matches("<insecureAdminAccess>true</insecureAdminAccess>")
                .count(),
            1
        );
    }
    let text = String::from_utf8(XML.to_vec())
        .unwrap()
        .replace("<insecureAdminAccess>false</insecureAdminAccess>", "");
    assert!(String::from_utf8(
        configure(text.as_bytes(), "fixture.example.invalid", true).unwrap()
    )
    .unwrap()
    .contains("<insecureAdminAccess>true</insecureAdminAccess>"));
}

#[test]
fn retained_shared_payloads_are_compatible_and_personal_scope_is_explicit() {
    let shared = settings();
    let payload = shared.payload().unwrap();
    assert!(!String::from_utf8_lossy(&payload).contains("instance"));
    assert_eq!(
        Settings::parse(&payload).unwrap().instance,
        Instance::Shared
    );
    let mut personal = shared;
    personal.instance = Instance::Personal;
    assert!(personal.validate().is_err());
    personal.device_name = "clamps-personal.example.invalid".into();
    assert_eq!(
        Settings::parse(&personal.payload().unwrap())
            .unwrap()
            .instance,
        Instance::Personal
    );
}
