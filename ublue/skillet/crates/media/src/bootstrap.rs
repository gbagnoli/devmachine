//! Seed consumer configuration atomically, preserving retained application state.
use crate::{App, Config, Credentials, Error};
use serde::Deserialize;
use skillet_core::files::{FileMutationResource, FileReadResource};

#[derive(Deserialize)]
struct Existing {
    #[serde(rename = "ApiKey")]
    api_key: String,
    #[serde(rename = "AuthenticationMethod")]
    authentication: String,
}

pub fn ensure<F: FileReadResource + FileMutationResource + ?Sized>(
    files: &F,
    config: Config,
    app: App,
    key: &str,
) -> Result<(), Error> {
    if !crate::valid_key(key) || app == App::Putioarr {
        return Err(Error::Invalid("bootstrap API key"));
    }
    let path = config.config_path(app).join("config.xml");
    if let Some(bytes) = files.read_file(&path)? {
        let existing: Existing = quick_xml::de::from_reader(bytes.as_slice())
            .map_err(|_| Error::Invalid("retained Arr config.xml"))?;
        if existing.api_key != key {
            return Err(Error::Invalid(
                "retained API key differs from the vault; recover it before applying",
            ));
        }
        if existing.authentication != "External" {
            return Err(Error::Invalid(
                "retained authentication mode is not External",
            ));
        }
        return Ok(());
    }
    let xml = format!("<Config><BindAddress>*</BindAddress><Port>{}</Port><EnableSsl>False</EnableSsl><LaunchBrowser>False</LaunchBrowser><ApiKey>{key}</ApiKey><AuthenticationMethod>External</AuthenticationMethod><AuthenticationRequired>Enabled</AuthenticationRequired><UrlBase></UrlBase><UpdateMechanism>External</UpdateMechanism><AnalyticsEnabled>False</AnalyticsEnabled></Config>\n", app.port());
    let owner = config.ownership();
    files.ensure_directory(&config.config_path(app), Some(0o700), &owner)?;
    if !files.create_file_if_absent(&path, xml.as_bytes(), Some(0o600), &owner)? {
        // A concurrent publisher must satisfy the same binding checks.
        return ensure(files, config, app, key);
    }
    Ok(())
}

/// JSON strings are valid TOML basic strings for these validated values.
pub fn putio_config(config: Config, credentials: &Credentials) -> Result<String, Error> {
    credentials.validate()?;
    let quoted = |value: &str| {
        serde_json::to_string(value).map_err(|_| Error::Invalid("putioarr configuration"))
    };
    let mut result = format!("username = \"skillet\"\npassword = {}\ndownload_directory = \"/data/downloads\"\nbind_address = \"0.0.0.0\"\nport = 9091\nuid = {}\ndownload_unmanaged = false\n[putio]\napi_key = {}\n", quoted(&credentials.putio_password)?, config.uid, quoted(&credentials.putio_token)?);
    for app in App::LIBRARIES {
        use std::fmt::Write as _;
        let _ = write!(
            result,
            "[{}]\nurl = {}\napi_key = {}\ncategory = \"{}\"\n",
            app.name(),
            quoted(&app.url())?,
            quoted(credentials.key(app)?)?,
            app.name()
        );
    }
    Ok(result)
}

#[cfg(test)]
#[path = "bootstrap/tests.rs"]
mod tests;
