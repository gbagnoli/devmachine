pub mod activation;
pub mod credential_install;
pub mod credentials;
pub mod files;
pub mod firewall;
pub mod maintenance;
pub mod recorder;
pub mod resource_op;
pub mod system;
pub mod templates;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
pub mod usb;
pub mod vendor_units;

#[cfg(test)]
#[path = "resource_contract_tests.rs"]
mod resource_contract_tests;
