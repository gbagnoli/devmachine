name "devmachinetest"
description "ubik test devmachine"
run_list [
  "role[devmachine]",
]

override_attributes(
  "user" => {
    "uid" => 2000,
    "gid" => 2000,
    "install_photo_process" => false,
  },
  "ubik" => {
    "skip_packages" => false,
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
