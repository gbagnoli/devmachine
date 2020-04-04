name "bendertest"
description "test bender"
run_list [
  "role[bender]",
]

override_attributes(
  "user" => {
    "uid" => 1500,
    "gid" => 1500,
  },
)
