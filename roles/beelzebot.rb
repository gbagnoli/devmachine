name "beelzebot"
description "configure beelzebot"
run_list [
  "role[server]",
  "role[container]",
  "recipe[beelzebot]",
]

default_attributes(
  "server" => {
    "components" => {
      "syncthing" => {
        "enabled" => false,
      },
      "user" => {
        "enabled" => false,
      },
    },
  },
)
