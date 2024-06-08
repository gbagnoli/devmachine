name "boxy"
description "Configure boxy"
run_list [
  "role[server]",
  "recipe[boxy]",
]

default_attributes(
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1003,
    "gid" => 1003,
    "realname" => "Giacomo Bagnoli",
    "install_vpnutils" => false,
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
  "chef_client_updater" => {
    "product_name" => "chef"
  }
)
