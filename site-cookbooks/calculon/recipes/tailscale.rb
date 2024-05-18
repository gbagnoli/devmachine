tsdir = node["calculon"]["storage"]["paths"]["tailscale"]
user = "nobody"
group = "nobody"

calculon_btrfs_volume tsdir do
  owner user
  group group
  mode "0700"
end

net = node["calculon"]["network"]["containers"]
node.override["tailscale"]["install_type"] = "podman"
node.override["tailscale"]["podman"]["user"] = user
node.override["tailscale"]["podman"]["group"] = group
node.override["tailscale"]["podman"]["config_dir"] = tsdir
node.override["tailscale"]["podman"]["extra_env"] = {
  "TS_ROUTES" => "#{net["ipv4"]["network"]},#{net["ipv6"]["network"]}",
  "TS_EXTRA_ARGS" => "--advertise-exit-node"
}

bash "enable masquerading" do
  code <<~EOH
    firewall-cmd --add-masquerade --zone=public
    firewall-cmd --permanent --add-masquerade --zone=public
  EOH
  not_if "firewall-cmd --query-masquerade --zone=public --permanent | grep -q yes"
end

calculon_firewalld_port "tailscaled" do
  port ["#{node["tailscale"]["port"]}/udp"]
end

include_recipe "tailscale::install"
