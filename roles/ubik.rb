name "ubik"
description "ubik workstation"
run_list [
  "role[devmachine]",
]

override_attributes(
  "user" => {
    "uid" => 1001,
    "gid" => 1001,
  },
  "users" => {
    "irene" => {
      "uid" => 1000,
      "gid" => 1000,
    },
  },
)
