name "bloodsea"
description "configure bloodsea"
run_list [
  "role[server]",
  "role[container]",
  "recipe[openvpn]",
]

default_attributes(
  "server" => {
    "components" => {
      "syncthing" => {
        "enabled" => true,
      },
      "user" => {
        "enabled" => false,
      },
    },
    "users" => {
      "dario" => {
        "unmanaged" => false,
      },
    },
  },
  # disable pam limits for pw changes
  "os-hardening" => {
    "auth" => {
      "pw_max_age" => -1,
      "pw_min_age" => -1,
      "pw_warn_age" => -1,
    }
  },
  "syncthing" => {
    "users" => {
      "dario" => {
        "hostname" => "syncthing.bloodsea.tigc.eu",
        "port" => 8384,
      },
    },
  },
)
