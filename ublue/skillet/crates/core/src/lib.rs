pub mod activation;
pub mod credential_install;
pub mod credentials;
pub mod files;
pub mod recorder;
pub mod resource_op;
pub mod system;
pub mod templates;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg(test)]
#[path = "resource_contract_tests.rs"]
mod resource_contract_tests;
