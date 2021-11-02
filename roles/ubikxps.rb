name "ubikxps"
description "ubik xps laptop"
run_list [
  "role[devmachine]",
  "recipe[ubik::intel]",
]

override_attributes(
  "os-hardening" => {
    "auth" => {
      "pw_max_age" => "-1",
      "pw_min_age" => "-1",
      "pw_warn_age" => "-1",
    }
  },
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
