name "ubik"
description "ubik workstation"
run_list [
  "role[devmachine]",
  # looks like it's not needed on 20.04?
  # "recipe[ubik::nvidia]",
]

override_attributes(
  "user" => {
    "uid" => 1000,
    "gid" => 1000,
  },
  "users" => {
    "irene" => {
      "uid" => 1001,
      "gid" => 1001,
    },
  },
)
