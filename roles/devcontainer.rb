name "devcontainer"
description "A role to configure a development container"
run_list [
  "recipe[user::default]",
  "recipe[ubik::ruby]",
  "recipe[ubik::golang]",
  "recipe[user::photos]",
]
default_attributes(
  "ubik" => {
    "golang" => {
      "version" => "1.23.3",
    },
    "ruby" => {
        "rubies" => ["3.3.0"],
        "user" => "giacomo",
    },
    "rust" => {
      "version" => "nightly"
    },
    "languages" => %w(en it),
    "install_latex" => true,
    "install_fonts" => true,
  },
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1000,
    "gid" => 1000,
    "realname" => "Giacomo Bagnoli",
    "homedir" => "/var/home",
  }
)
