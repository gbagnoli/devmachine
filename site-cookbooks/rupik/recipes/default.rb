include_recipe "podman::install"
include_recipe "argonone"
include_recipe "rupik::mounts"
include_recipe "rupik::btrbk"
include_recipe "rupik::pihole"
include_recipe "rupik::unifi"
include_recipe "syncthing"

node.override["tailscale"]["install_type"] = "podman"
node.override["tailscale"]["podman"]["user"] = "nobody"
node.override["tailscale"]["podman"]["group"] = "nogroup"
node.override["tailscale"]["podman"]["config_dir"] = "#{node["rupik"]["storage"]["path"]}/tailscale"
node.override["tailscale"]["podman"]["extra_env"] = {
  "TS_ROUTES" => node["rupik"]["ipv4"]["network"],
  "TS_EXTRA_ARGS" => "--advertise-exit-node"
}
include_recipe "tailscale::install"
