use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Environment {
    Production,
    Test,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceClass {
    ProductionHost,
    DisposableVm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisioningPolicy {
    environment: Environment,
}

impl ProvisioningPolicy {
    pub const fn new(environment: Environment) -> Self {
        Self { environment }
    }

    pub const fn environment(self) -> Environment {
        self.environment
    }

    pub const fn name(self) -> &'static str {
        match self.environment {
            Environment::Production => "production",
            Environment::Test => "test",
        }
    }

    pub const fn vault_name(self) -> &'static str {
        match self.environment {
            Environment::Production => "prod",
            Environment::Test => "test",
        }
    }

    pub fn ui_domain_entry(self) -> String {
        format!("skillet/environments/{}/dns/ui-domain", self.vault_name())
    }

    pub fn cloudflare_zone_entry(self) -> String {
        format!(
            "skillet/environments/{}/dns/cloudflare-zone-id",
            self.vault_name()
        )
    }

    pub const fn acme_staging(self) -> bool {
        matches!(self.environment, Environment::Test)
    }

    pub const fn cloudflare_token_lifetime(self) -> Option<Duration> {
        match self.environment {
            Environment::Production => None,
            Environment::Test => Some(Duration::from_hours(12)),
        }
    }

    pub const fn cleanup_token_lifetime(self) -> Duration {
        Duration::from_mins(15)
    }

    pub const fn tailscale_tag(self, device: DeviceClass) -> &'static str {
        match device {
            DeviceClass::ProductionHost => crate::tailscale::SERVER_TAG,
            DeviceClass::DisposableVm => crate::tailscale::SMOKE_TAG,
        }
    }
}

#[cfg(test)]
#[path = "provisioning_policy/tests.rs"]
mod tests;
