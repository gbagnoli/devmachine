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
            self.tests.set(self.tests.get() + 1);
            return Ok(json!([]));
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
            Method::Get => Ok(json!(entries)),
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
                *existing = value.clone();
                self.writes.set(self.writes.get() + 1);
                Ok(value.clone())
            }
        }
    }
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
