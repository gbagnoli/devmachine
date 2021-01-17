name "ubik"
description "ubik workstation"
run_list [
  "role[devmachine]",
]

override_attributes(
  "user" => {
    "uid" => 1000,
    "gid" => 1000,
  },
  "users" => {
    "irene" => {
      "uid" => 1001,
      "gid" => 1001,
    },
  },
  "usb" => {
    "always_on_devices": {
      "hub": {idVendor: "05e3", "idProduct": "0610"},
      "razor": {idVendor: "1532", "idProduct": "0065"},
      "k3": {idVendor: "2972", "idProduct": "0047"},
    }
  }
)
