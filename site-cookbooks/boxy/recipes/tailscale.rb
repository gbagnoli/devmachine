node.override["tailscale"]["install_type"] = "podman"
node.override["tailscale"]["podman"]["user"] = "nobody"
node.override["tailscale"]["podman"]["group"] = "nogroup"
node.override["tailscale"]["podman"]["config_dir"] = "#{node["boxy"]["storage"]["path"]}/tailscale"
node.override["tailscale"]["podman"]["export_resolv.conf"] = true
node.override["tailscale"]["podman"]["extra_env"] = {
  "TS_ROUTES" => node["boxy"]["lan"]["ipv4"]["network"],
  "TS_EXTRA_ARGS" => '"--advertise-exit-node --accept-routes --snat-subnet-routes=false"',
  "TS_ACCEPT_DNS" => "false",
}
include_recipe "tailscale::install"
