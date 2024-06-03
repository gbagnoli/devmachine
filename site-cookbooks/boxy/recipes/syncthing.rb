node.override["syncthing"]["install_type"] = "podman"
node.override["syncthing"]["podman"] = {
  "directory" => "#{node["boxy"]["storage"]["path"]}/sync",
  "uid" => node["user"]["uid"],
  "gid" => node["user"]["gid"],
  "ipv6" => {
    "gui" => "::",
    "service" => "::",
  },
  "ipv4" => {
    "gui" => "",
    "service" => "",
  },
  "extra_conf" => %w{
    Hostname=boxy.tigc.eu
  }
}

include_recipe "syncthing::default"
