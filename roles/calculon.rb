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
  "server" => {
    "users" => {
      "dario" => {
        "unmanaged" => true,
      },
    },
  },
)
