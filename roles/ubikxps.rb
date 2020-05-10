name "ubikxps"
description "ubik xps laptop"
run_list [
  "role[devmachine]",
  "recipe[ubik::xps]",
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
