name "calculon"
description "configure calculon"
run_list [
  "role[server]",
  "recipe[calculon]",
]

default_attributes(
  "server" => {
    "components" => {
      "syncthing" => {
        "enabled" => "false"
      },
    },
    "users" => {
      "dario" => {
        "unmanaged" => true,
      },
    },
  },
)
