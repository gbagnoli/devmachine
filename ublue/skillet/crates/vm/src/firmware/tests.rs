use super::*;
#[test]
fn missing_tpm_never_falls_back() {
    let xml="<domainCapabilities><os><enum name='firmware'><value>efi</value></enum><loader><enum name='secure'><value>yes</value></enum></loader></os><devices><tpm supported='no'/></devices></domainCapabilities>";
    assert!(verify_capabilities(xml, "Running against daemon: 11.10.0").is_err());
    let xml=xml.replace("supported='no'", "supported='yes'").replace("<tpm supported='yes'/>", "<tpm supported='yes'><enum name='backendModel'><value>emulator</value></enum><enum name='backendVersion'><value>2.0</value></enum></tpm>");
    assert!(verify_capabilities(&xml, "Running against daemon: 11.10.0").is_ok());
    assert!(verify_capabilities(&xml, "Running against daemon: 10.9.0").is_err());
}
