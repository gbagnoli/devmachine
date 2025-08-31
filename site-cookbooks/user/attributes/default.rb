# frozen_string_literal: true

default["user"]["login"] = "giacomo"
default["user"]["group"] = "giacomo"
default["user"]["homedir"] = "/home"
default["user"]["uid"] = 1000
default["user"]["gid"] = 1000
default["user"]["realname"] = "Giacomo Bagnoli"
default["user"]["install_vpnutils"] = false
default["user"]["install_photo_process"] = true
default["user"]["ssh_authorized_keys"] = [{
  name: "giacomo@ubikxps",
  keytype: "ssh-ed25519",
  pubkey: "AAAAC3NzaC1lZDI1NTE5AAAAICYS/jhr4/Ld55BT2YjP+b+LHWNkaSuRYSuLre2Mbxwz",
}, {
  name: "giacomo@ubik",
  pubkey: "AAAAC3NzaC1lZDI1NTE5AAAAIG5BCzEZkae5DssrD+FFi0a4YGYT55b57LbXW8SveytN",
  keytype: "ssh-ed25519",
}, {
  name: "giacomo@android",
  pubkey: "AAAAC3NzaC1lZDI1NTE5AAAAIGMkGTp3emesafdNNkprN2Hx1i2wLER9RUhWguwTLu6+",
  keytype: "ssh-ed25519",
}, {
  name: "giacomo@calculon",
  pubkey: "AAAAC3NzaC1lZDI1NTE5AAAAID19B1eeB6KClwXbmu+RWo+3nrXl33Yf3+Oh/WVigrwl",
  keytype: "ssh-ed25519",
},{
  name: "giacomo@mac",
  pubkey: "AAAAC3NzaC1lZDI1NTE5AAAAIB+LUBD9jvN98k6es/W/0h/nGibhHGf7OY6JZG+H3pQL",
  keytype: "ssh-ed25519",
}]

# must be set in secrets
default["gphotos_uploader_cli"] = nil
# default["gphotos_uploader_cli"]["ClientID"] = nil
# default["gphotos_uploader_cli"]["ClientSecret"] = nil
# default["gphotos_uploader_cli"]["Account"] = nil
