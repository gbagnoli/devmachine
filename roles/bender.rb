name "bender"
description "configure bender"
run_list [
  "role[server]",
  "recipe[bender::default]",
]

default_attributes(
  "server" => {
    "users" => {
      "fnigi" => {
        "unmanaged" => false,
      },
      "dario" => {
        "unmanaged" => false,
      },
      "sonne" => {
        "unmanaged" => false,
      },
    },
  },
)
