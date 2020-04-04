name "ubiktest"
description "ubik test"
run_list [
  "role[devlaptop]",
]

default_attributes(
  "user" => {
    "uid" => 2000,
    "gid" => 2000,
    "install_vpnutils" => false,
  },
  "ubik" => {
    "enable_mtrack" => false,
    "skip_packages" => true,
    "install_latex" => false,
    "install_fonts" => false,
  },
  "syncthing" => {
    "skip_service" => true,
  },
  "users" => {
    "irene" => {
      "uid" => 2001,
      "gid" => 2001,
    },
  },
)
