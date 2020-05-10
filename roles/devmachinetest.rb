name "devmachinetest"
description "ubik test devmachine"
run_list [
  "role[devmachine]",
]

override_attributes(
  "user" => {
    "uid" => 2000,
    "gid" => 2000,
    "install_vpnutils" => false,
  },
  "ubik" => {
    "skip_packages" => false,
    "install_latex" => true,
    "install_fonts" => true,
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

