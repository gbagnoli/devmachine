name "marvintest"
description "test marvin"
run_list [
  "role[marvin]",
]

override_attributes(
  "user" => {
    "uid" => 1500,
    "gid" => 1500,
  },
  "server" => {
    "components" => {
      "chef_client_updater" => {
        "enabled" => false
      }
    }
  }
)
