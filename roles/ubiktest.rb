name "ubiktest"
description "ubik test"
run_list [
  "role[devmachine]",
]

default_attributes(
  "user" => {
    "uid" => 2000,
    "gid" => 2000,
  },
  "ubik" => {
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
