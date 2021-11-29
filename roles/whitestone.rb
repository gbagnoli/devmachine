name "whitestone"
description "configure whitestone"
run_list [
  "role[server]",
  "role[container]",
  "recipe[minecraft::default]",
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
        "hostname" => "syncthing.whitestone.tigc.eu",
        "port" => 8384,
      },
    },
  },
  "minecraft" => {
    "group_users" => %w[giacomo dario],
    "server" => {
      "properties" => {
        # password in secrets.json
        # add extra conf here
        "server-ip" => "172.24.24.7"
      }
    }
  }
)
