name "devcontainer"
description "A role to configure a development container"
run_list [
  "recipe[user::default]",
  "recipe[ubik::python]",
  "recipe[ubik::ruby]",
  # "recipe[ubik::rust]",
  "recipe[ubik::golang]",
  "recipe[ubik::java]",
]
default_attributes(
  "ubik" => {
    "golang" => {
      "version" => "1.22.2",
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
      "versions" => ["2.7.18", "3.12.2"],
      "user_global" => "3.12.2 2.7.18",
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
    "install_vpnutils" => false,
  }
)
