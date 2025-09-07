name "boxy"
description "Configure boxy"
run_list [
  "role[server]",
  "recipe[server::wol]",
  "recipe[boxy]",
]

default_attributes(
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1003,
    "gid" => 1003,
    "realname" => "Giacomo Bagnoli",
  },
)

override_attributes(
  "apt" => {
    "unattended_upgrades" => {
      "allowed_origins" => [
        "${distro_id}:${distro_codename}",
        "${distro_id}:${distro_codename}-security",
        "${distro_id}ESMApps:${distro_codename}-apps-security",
        "${distro_id}ESM:${distro_codename}-infra-security",
      ],
    },
  },
  "sysctl" => {
    "params" => {
      "net" => {
        "ipv4" => {
          "conf" => {
            "all" => {
              "log_martians" => 0
            },
            "default" => {
              "log_martians" => 0
            },
          },
        },
      },
    },
  },
  "chef_client_updater" => {
    "product_name" => "chef"
  }
)
