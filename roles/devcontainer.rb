name "devcontainer"
description "A role to configure a development container"
run_list [
  "recipe[user::default]",
  "recipe[user::photos]",
]
default_attributes(
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1000,
    "gid" => 1000,
    "realname" => "Giacomo Bagnoli",
    "homedir" => "/var/home",
  }
)
