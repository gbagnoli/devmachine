name "oldpc"
description "old people pc"
run_list [
  "user",
]

override_attributes(
  "user" => {
    "uid" => 1001,
    "gid" => 1001,
    "login" => "jdoe",
    "group" => "jdoe",
  },
)
