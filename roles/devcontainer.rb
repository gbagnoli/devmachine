name "devcontainer"
description "A role to configure a development container"
run_list [
  "recipe[user::default]",
  "recipe[ubik::python]",
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
    "python" => {
      "user" => "giacomo",
      "versions" => ["2.7.18", "3.13.0"],
      "user_global" => "3.13.0 2.7.18",
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
