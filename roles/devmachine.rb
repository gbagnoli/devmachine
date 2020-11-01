name "devmachine"
description "A role to configure a development workstation"
run_list [
  "recipe[ubik::ppa]",
  "recipe[hardening]",
  "recipe[ubik::users]",
  "recipe[ubik::python]",
  "recipe[ubik::ruby]",
  "recipe[ubik::rust]",
  "recipe[ubik::golang]",
  "recipe[ubik::java]",
  "recipe[ubik::ubuntu_hwe]",
  "recipe[ubik::packages]",
  "recipe[ubik::printer]",
  "recipe[syncthing]",
  "recipe[openvpn]",
  "recipe[ubik::langs]",
  "recipe[ubik::latex]",
  "recipe[ubik::fonts]",
  "recipe[ubik::gnome_extensions]",
]
default_attributes(
  "authorization" => {
    "sudo" => {
      "include_sudoers_d" => true,
    },
  },
  "ubik" => {
    "golang" => {
      "version" => "1.15.3",
    },
    "ruby" => {
        "rubies" => ["2.5.8", "2.7.2"],
        "user" => "giacomo",
    },
    "rust" => {
      "version" => "nightly"
    },
    "python" => {
      "user" => "giacomo",
      "versions" => ["2.7.17", "3.9.0"],
      "user_global" => "3.9.0 2.7.17",
    },
    "languages" => %w[en it],
    "install_latex" => true,
    "install_fonts" => true,
  },
  "os-hardening" => {
    "auth" => {
      "retries" => 15,
      "lockout_retries" => 300,
      "timeout" => 120,
    },
    "desktop" => {
      "enable" => true,
    },
    "network" => {
      "ipv6" => {
        "enable" => true,
      },
    },
    "security" => {
      "kernel" => {
        "enable_module_loading" => true,
        "disable_filesystems" => %w[cramfs freevxfs jffs2 hfs
                                    hfsplus squashfs udf],
      },
    },
  },
  "syncthing" => {
    "users" => {
      "giacomo" => nil,
      "irene" => {
        "hostname" => "ubik-irene",
        "port" => 8385,
      },
    },
  },
  "user" => {
    "login" => "giacomo",
    "group" => "giacomo",
    "uid" => 1000,
    "gid" => 1000,
    "realname" => "Giacomo Bagnoli",
    "install_vpnutils" => true,
  },
)
