//! Fixed fleet roles keep service, bootstrap and listener identities together.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Instance {
    #[default]
    Shared,
    Personal,
}

impl Instance {
    pub fn is_shared(&self) -> bool {
        *self == Self::Shared
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Shared => "syncthing",
            Self::Personal => "syncthing-personal",
        }
    }
    pub fn service(self) -> &'static str {
        match self {
            Self::Shared => "syncthing.service",
            Self::Personal => "syncthing-personal.service",
        }
    }
    pub fn credential(self) -> &'static str {
        match self {
            Self::Shared => "syncthing_initial",
            Self::Personal => "syncthing_personal_initial",
        }
    }
    pub fn initial_unit(self) -> &'static str {
        match self {
            Self::Shared => "skillet-syncthing-initialize.service",
            Self::Personal => "skillet-syncthing-personal-initialize.service",
        }
    }
    pub fn peer_port(self) -> u16 {
        match self {
            Self::Shared => 22000,
            Self::Personal => 22202,
        }
    }
    pub fn device_label(self, runtime_hostname: &str) -> String {
        match self {
            Self::Shared => runtime_hostname.into(),
            Self::Personal => format!("{runtime_hostname}-personal"),
        }
    }
}
