use super::*;
use crate::fixtures::{credentials, CONFIG};
use skillet_core::test_utils::MockFiles;

#[test]
fn bootstrap_retains_app_edits_and_refuses_key_replacement() {
    let files = MockFiles::new();
    let key = credentials().key(App::Sonarr).unwrap().to_string();
    ensure(&files, CONFIG, App::Sonarr, &key).unwrap();
    let path = CONFIG.config_path(App::Sonarr).join("config.xml");
    let content = String::from_utf8(files.read_file(&path).unwrap().unwrap()).unwrap();
    let edited = content.replace(
        "</Config>",
        "<OperatorSetting>keep-me</OperatorSetting></Config>",
    );
    files
        .ensure_file(
            &path,
            edited.as_bytes(),
            None,
            &skillet_core::files::Ownership::default(),
        )
        .unwrap();
    ensure(&files, CONFIG, App::Sonarr, &key).unwrap();
    assert_eq!(files.read_file(&path).unwrap().unwrap(), edited.as_bytes());
    assert!(ensure(&files, CONFIG, App::Sonarr, &"f".repeat(32)).is_err());
    assert_eq!(files.read_file(&path).unwrap().unwrap(), edited.as_bytes());
}

#[test]
fn putioarr_callbacks_include_all_three_categories_and_private_names() {
    let config = putio_config(CONFIG, &credentials()).unwrap();
    for app in App::LIBRARIES {
        assert!(config.contains(&format!("[{}]", app.name())));
        assert!(config.contains(&app.url()));
        assert!(config.contains(&format!("category = \"{}\"", app.name())));
    }
    assert!(config.contains("download_unmanaged = false"));
}
