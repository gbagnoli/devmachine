use super::*;
#[test]
fn test_payload_never_reads_provider_credentials() {
    let input = input(ProvisioningPolicy::new(Environment::Test), &|_, _| {
        panic!("test must not open the vault")
    })
    .unwrap();
    assert_eq!(input.payload().unwrap(), r#"{"mode":"capture"}"#);
}
#[test]
fn production_requires_all_fields_without_echoing_values() {
    let policy = ProvisioningPolicy::new(Environment::Production);
    for missing in ["host", "port", "tls", "UserName", "Password", "sender"] {
        let result = input(policy, &|_, field| {
            Ok(if field == missing {
                None
            } else {
                Some(
                    match field {
                        "host" => "smtp.example.com",
                        "port" => "587",
                        "tls" => "starttls",
                        "sender" => "server@example.com",
                        _ => "fixture-secret",
                    }
                    .into(),
                )
            })
        });
        let Err(error) = result else {
            panic!("missing field accepted")
        };
        assert!(!error.to_string().contains("fixture-secret"));
    }
}
