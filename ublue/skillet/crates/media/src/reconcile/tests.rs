use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

#[derive(Default)]
struct Fixture {
    values: RefCell<BTreeMap<(App, String), Vec<Value>>>,
    writes: Cell<usize>,
    fail_after_create: Cell<bool>,
    tests: Cell<usize>,
}

impl Api for Fixture {
    fn request(
        &self,
        app: App,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, Error> {
        if path.ends_with("/schema") {
            let implementations = if path.starts_with("applications") {
                vec!["Sonarr", "Radarr", "Lidarr"]
            } else {
                vec!["Transmission"]
            };
            let fields = [
                "prowlarrUrl",
                "baseUrl",
                "apiKey",
                "host",
                "port",
                "useSsl",
                "urlBase",
                "username",
                "password",
                "tvCategory",
                "movieCategory",
                "musicCategory",
            ]
            .map(|name| json!({"name":name,"value":null}));
            return Ok(json!(implementations.into_iter().map(|name| json!({"implementation":name,"configContract":format!("{name}Settings"),"fields": fields})).collect::<Vec<_>>()));
        }
        if path.ends_with("/test") {
            return self.test_provider(app, path, body);
        }
        if path == "system/status" {
            return Ok(json!({"version":"fixture"}));
        }
        if path == "qualityprofile" || path == "metadataprofile" {
            return Ok(json!([{"id":1}]));
        }
        let collection = path.split('/').next().unwrap().to_string();
        let mut values = self.values.borrow_mut();
        let entries = values.entry((app, collection)).or_default();
        match method {
            Method::Get => {
                let mut response = json!(entries);
                for entry in response.as_array_mut().unwrap() {
                    if let Some(fields) = entry["fields"].as_array_mut() {
                        for field in fields {
                            if matches!(field["name"].as_str(), Some("apiKey" | "password"))
                                && field["value"].as_str().is_some_and(|v| !v.is_empty())
                            {
                                field["value"] = json!("********");
                            }
                        }
                    }
                }
                Ok(response)
            }
            Method::Post => {
                let mut value = body.unwrap().clone();
                value["id"] = json!(entries.len() + 1);
                entries.push(value.clone());
                self.writes.set(self.writes.get() + 1);
                if path == "downloadclient" && self.fail_after_create.replace(false) {
                    return Err(Error::Invalid("injected lost response"));
                }
                Ok(value)
            }
            Method::Put => {
                let value = body.unwrap();
                let existing = entries
                    .iter_mut()
                    .find(|entry| entry["id"] == value["id"])
                    .unwrap();
                let mut update = value.clone();
                for field in update["fields"].as_array_mut().unwrap() {
                    if matches!(field["name"].as_str(), Some("apiKey" | "password"))
                        && field["value"] == "********"
                    {
                        field["value"] = field_value(existing, field["name"].as_str().unwrap());
                    }
                }
                *existing = update;
                self.writes.set(self.writes.get() + 1);
                Ok(value.clone())
            }
        }
    }
}

impl Fixture {
    fn test_provider(&self, app: App, path: &str, body: Option<&Value>) -> Result<Value, Error> {
        let collection = path.strip_suffix("/test").unwrap();
        let state = self.values.borrow();
        let existing = state
            .get(&(app, collection.to_string()))
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry["name"] == body.unwrap()["name"])
            });
        if let Some(existing) = existing {
            if existing["id"] != body.unwrap()["id"] {
                return Err(Error::Invalid("existing provider test omitted its ID"));
            }
        }
        let (name, expected) = if collection == "applications" {
            let target = App::LIBRARIES
                .into_iter()
                .find(|target| body.unwrap()["implementation"] == target.implementation())
                .unwrap();
            (
                "apiKey",
                json!(crate::fixtures::credentials().key(target).unwrap()),
            )
        } else {
            (
                "password",
                json!(crate::fixtures::credentials().putio_password),
            )
        };
        let mut supplied = field_value(body.unwrap(), name);
        if supplied == "********" {
            supplied = existing.map_or(Value::Null, |value| field_value(value, name));
        }
        if supplied != expected {
            return Err(Error::Api {
                app: app.name(),
                operation: format!("test {collection}"),
                status: 400,
            });
        }
        self.tests.set(self.tests.get() + 1);
        Ok(json!([]))
    }
}

fn field_value(value: &Value, name: &str) -> Value {
    value["fields"]
        .as_array()
        .and_then(|fields| fields.iter().find(|field| field["name"] == name))
        .map_or(Value::Null, |field| field["value"].clone())
}

#[test]
fn masked_credentials_are_verified_and_drifted_credentials_are_repaired_once() {
    let fixture = Fixture::default();
    let credentials = crate::fixtures::credentials();
    reconcile(&fixture, &credentials).unwrap();
    let before = fixture.writes.get();
    reconcile(&fixture, &credentials).unwrap();
    assert_eq!(fixture.writes.get(), before);
    {
        let mut state = fixture.values.borrow_mut();
        let application = &mut state
            .get_mut(&(App::Prowlarr, "applications".into()))
            .unwrap()[0];
        application["fields"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|field| field["name"] == "apiKey")
            .unwrap()["value"] = json!("f".repeat(32));
    }
    reconcile(&fixture, &credentials).unwrap();
    assert_eq!(fixture.writes.get(), before + 1);
    reconcile(&fixture, &credentials).unwrap();
    assert_eq!(fixture.writes.get(), before + 1);
}

#[test]
fn repeat_and_interrupted_reconciliation_preserve_unrelated_state_without_duplicates() {
    let fixture = Fixture::default();
    fixture.values.borrow_mut().insert(
        (App::Sonarr, "downloadclient".into()),
        vec![json!({"name":"Operator client","implementation":"Other","id":41})],
    );
    fixture.fail_after_create.set(true);
    assert!(reconcile(&fixture, &crate::fixtures::credentials()).is_err());
    reconcile(&fixture, &crate::fixtures::credentials()).unwrap();
    let writes = fixture.writes.get();
    reconcile(&fixture, &crate::fixtures::credentials()).unwrap();
    assert_eq!(fixture.writes.get(), writes);
    let values = fixture.values.borrow();
    assert_eq!(values[&(App::Prowlarr, "applications".into())].len(), 3);
    assert_eq!(values[&(App::Sonarr, "downloadclient".into())].len(), 2);
    assert_eq!(
        values[&(App::Sonarr, "downloadclient".into())][0]["name"],
        "Operator client"
    );
    assert!(fixture.tests.get() >= 12);
}

#[test]
fn ambiguous_owned_names_fail_without_additional_mutations() {
    let fixture = Fixture::default();
    reconcile(&fixture, &crate::fixtures::credentials()).unwrap();
    let owned = fixture.values.borrow()[&(App::Sonarr, "downloadclient".into())][0].clone();
    fixture
        .values
        .borrow_mut()
        .get_mut(&(App::Sonarr, "downloadclient".into()))
        .unwrap()
        .push(owned);
    let writes = fixture.writes.get();
    assert!(reconcile(&fixture, &crate::fixtures::credentials()).is_err());
    assert_eq!(fixture.writes.get(), writes);
}
