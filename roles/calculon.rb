name "calculon"
description "configure calculon"
run_list [
  "role[server]",
  "recipe[calculon]",
]

default_attributes(
  "server" => {
    "users" => {
      "dario" => {
        "unmanaged" => true,
      },
    },
  },
)
