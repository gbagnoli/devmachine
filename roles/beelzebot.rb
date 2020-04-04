name "beelzebot"
description "configure pentolino"
run_list [
  "role[server]",
  "role[container]",
  "recipe[beelzebot::default]",
]

default_attributes(
  "server" => {
    "components" => {
      "syncthing" => {
        "enabled" => false,
      },
    },
  },
)
