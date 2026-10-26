//! Consumer-schema-driven reconciliation, bounded to reserved Skillet names.
use crate::{App, Credentials, Error};
use serde_json::{json, Value};

#[derive(Clone, Copy)]
pub enum Method {
    Get,
    Post,
    Put,
}

pub trait Api {
    fn request(
        &self,
        app: App,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, Error>;
}

/// Keys are selected before startup; only owned links and missing root folders
/// are reconciled. User libraries, indexers and unrelated links are retained.
pub fn reconcile(api: &impl Api, credentials: &Credentials) -> Result<(), Error> {
    reconcile_applications(api, credentials)?;
    for app in App::LIBRARIES {
        download_client(api, app, credentials)?;
    }
    Ok(())
}

/// Reconcile the Arr-to-Arr links independently of an external download account.
/// Disposable consumers can validate real API schemas without a download account.
pub fn reconcile_applications(api: &impl Api, credentials: &Credentials) -> Result<(), Error> {
    credentials.validate()?;
    for app in App::ARR {
        api.request(app, Method::Get, "system/status", None)?;
    }
    for app in App::LIBRARIES {
        root(api, app)?;
        application(api, app, credentials)?;
    }
    Ok(())
}

fn list(value: &Value) -> Result<&[Value], Error> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("application API schema"))
}

fn schema(
    api: &impl Api,
    app: App,
    collection: &str,
    implementation: &str,
) -> Result<Value, Error> {
    let response = api.request(app, Method::Get, &format!("{collection}/schema"), None)?;
    list(&response)?
        .iter()
        .find(|value| value["implementation"] == implementation)
        .cloned()
        .ok_or(Error::Invalid("required API implementation is unavailable"))
}

fn field(value: &mut Value, name: &str, desired: Value) -> Result<(), Error> {
    let entry = value["fields"]
        .as_array_mut()
        .and_then(|fields| fields.iter_mut().find(|entry| entry["name"] == name))
        .ok_or(Error::Invalid("required consumer field is unavailable"))?;
    entry["value"] = desired;
    Ok(())
}

fn download_client(api: &impl Api, app: App, credentials: &Credentials) -> Result<(), Error> {
    let mut value = schema(api, app, "downloadclient", "Transmission")?;
    value["name"] = json!("Skillet putioarr");
    value["enable"] = json!(true);
    value["priority"] = json!(1);
    let category = match app {
        App::Sonarr => "tvCategory",
        App::Radarr => "movieCategory",
        App::Lidarr => "musicCategory",
        _ => return Err(Error::Invalid("download client application")),
    };
    for (name, desired) in [
        ("host", json!("putioarr")),
        ("port", json!(9091)),
        ("useSsl", json!(false)),
        ("urlBase", json!("/transmission/")),
        ("username", json!("skillet")),
        ("password", json!(credentials.putio_password)),
        (category, json!(app.name())),
    ] {
        field(&mut value, name, desired)?;
    }
    upsert(
        api,
        app,
        "downloadclient",
        &value,
        &[
            "host", "port", "useSsl", "urlBase", "username", "password", category,
        ],
        &["enable", "priority"],
    )
}

fn application(api: &impl Api, app: App, credentials: &Credentials) -> Result<(), Error> {
    let mut value = schema(api, App::Prowlarr, "applications", app.implementation())?;
    value["name"] = json!(format!("Skillet {}", app.implementation()));
    value["syncLevel"] = json!("fullSync");
    for (name, desired) in [
        ("prowlarrUrl", json!(App::Prowlarr.url())),
        ("baseUrl", json!(app.url())),
        ("apiKey", json!(credentials.key(app)?)),
    ] {
        field(&mut value, name, desired)?;
    }
    upsert(
        api,
        App::Prowlarr,
        "applications",
        &value,
        &["prowlarrUrl", "baseUrl", "apiKey"],
        &["syncLevel"],
    )
}

fn test(api: &impl Api, app: App, collection: &str, value: &Value) -> Result<(), Error> {
    let result = api.request(
        app,
        Method::Post,
        &format!("{collection}/test"),
        Some(value),
    )?;
    if result.as_array().is_some_and(|errors| !errors.is_empty()) {
        return Err(Error::Invalid("consumer connection test failed"));
    }
    Ok(())
}

fn upsert(
    api: &impl Api,
    app: App,
    collection: &str,
    desired: &Value,
    fields: &[&str],
    properties: &[&str],
) -> Result<(), Error> {
    let response = api.request(app, Method::Get, collection, None)?;
    let owned = list(&response)?
        .iter()
        .filter(|value| value["name"] == desired["name"])
        .collect::<Vec<_>>();
    if owned.len() > 1 {
        return Err(Error::Invalid("duplicate reserved Skillet link name"));
    }
    if let Some(existing) = owned.first() {
        if existing["implementation"] != desired["implementation"] {
            return Err(Error::Invalid(
                "reserved Skillet link has a different implementation",
            ));
        }
        let mut update = (*existing).clone();
        let mut masked = Vec::new();
        for property in properties {
            update[*property] = desired[*property].clone();
        }
        for name in fields {
            let wanted = list(&desired["fields"])?
                .iter()
                .find(|v| v["name"] == *name)
                .ok_or(Error::Invalid("desired link field"))?;
            let current = list(&existing["fields"])?
                .iter()
                .find(|value| value["name"] == *name)
                .ok_or(Error::Invalid("retained link field"))?;
            if matches!(*name, "apiKey" | "password") && current["value"] == "********" {
                masked.push((*name, wanted["value"].clone()));
            } else {
                field(&mut update, name, wanted["value"].clone())?;
            }
        }
        match test(api, app, collection, &update) {
            Ok(()) => {}
            Err(Error::Api { status: 400, .. }) if !masked.is_empty() => {
                // The provider resolves its documented mask against this ID's
                // stored secret. Only repair after the expected secret passes
                // the same real consumer connection test.
                for (name, value) in masked {
                    field(&mut update, name, value)?;
                }
                test(api, app, collection, &update)?;
            }
            Err(error) => return Err(error),
        }
        if update != **existing {
            let id = existing["id"]
                .as_u64()
                .ok_or(Error::Invalid("owned link identifier"))?;
            api.request(
                app,
                Method::Put,
                &format!("{collection}/{id}"),
                Some(&update),
            )?;
        }
    } else {
        test(api, app, collection, desired)?;
        api.request(app, Method::Post, collection, Some(desired))?;
    }
    Ok(())
}

fn root(api: &impl Api, app: App) -> Result<(), Error> {
    let path = app.root().ok_or(Error::Invalid("root application"))?;
    let response = api.request(app, Method::Get, "rootfolder", None)?;
    if list(&response)?.iter().any(|value| value["path"] == path) {
        return Ok(());
    }
    let mut value = json!({"path": path});
    if app == App::Lidarr {
        value["name"] = json!("Music");
        for (endpoint, name) in [
            ("qualityprofile", "defaultQualityProfileId"),
            ("metadataprofile", "defaultMetadataProfileId"),
        ] {
            let response = api.request(app, Method::Get, endpoint, None)?;
            value[name] = list(&response)?
                .first()
                .and_then(|value| value.get("id"))
                .cloned()
                .ok_or(Error::Invalid("Lidarr default profile"))?;
        }
        value["defaultMonitorOption"] = json!("all");
        value["defaultNewItemMonitorOption"] = json!("all");
    }
    api.request(app, Method::Post, "rootfolder", Some(&value))?;
    Ok(())
}

#[cfg(test)]
#[path = "reconcile/tests.rs"]
mod tests;
