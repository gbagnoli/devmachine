name "marvin"
description "configure marvin"
run_list [
  "role[server]",
  "role[container]",
  "recipe[marvin::default]",
  "recipe[tailscale::install]",
]

default_attributes(
  "syncthing" => {
    "users" => {
      "giacomo" => {
        "hostname" => "syncthing.tigc.eu",
        "port" => 8384,
      },
    },
  },
)
