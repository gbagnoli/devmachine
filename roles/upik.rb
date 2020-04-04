name "upik"
description "Configure upik"
run_list [
  "role[server]",
  "recipe[btrbk]",
  "recipe[upik::mounts]",
  "recipe[upik::default]",
  "recipe[dnscrypt_proxy]",
]

default_attributes(
  "syncthing" => {
    "users" => {
      "up" => nil,
    },
  },
  "user" => {
    "login" => "up",
    "group" => "up",
    "uid" => 1000,
    "gid" => 1000,
    "realname" => "ubik",
  },
)

override_attributes(
  "apt" => {
    "unattended_upgrades" => {
      "allowed_origins" => [
        "Debian:stable",
        "Debian:stable-updates",
        "Syncthing:syncthing",
        "ubilinux:ubilinux3-upboard",
        ". wheezy:wheezy",
      ],
    },
  },
  "upik" => {
    "skip_mounts" => false,
  },
)
