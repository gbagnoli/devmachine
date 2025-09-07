name "rupik"
description "Configure rupik"
run_list [
  "role[server]",
  "recipe[server::wol]",
  "recipe[rupik]",
]

default_attributes(
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1000,
    "gid" => 1000,
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
        "Syncthing:syncthing",
      ],
    },
  },
)
