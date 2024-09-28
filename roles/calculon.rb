name "calculon"
description "configure calculon"
run_list [
  "role[server]",
  "recipe[calculon]",
]

default_attributes(
  "ssh-hardening" => {
    "sshclient" => {
      "package" => "openssh-clients"
    }
  },
  "chef_client_updater" => {
    "product_name" => "cinc-workstation",
    "version" => "latest"
  },
  "server" => {
    "components" => {
      "chef_client_updater" => {
        "enabled" => false
      }
    },
    "users" => {
      "dario" => {
        "unmanaged" => true,
      },
    },
  },
)
