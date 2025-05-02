tsdir = node["calculon"]["storage"]["paths"]["tailscale"]
phdir = node["calculon"]["storage"]["paths"]["pihole"]
user = "nobody"
group = "nobody"

[tsdir, phdir].each do |vol|
  calculon_btrfs_volume vol do
    owner user
    group group
    mode "0700"
  end
end

%w{conf log}.each do |dir|
  directory "#{phdir}/#{dir}" do
    owner user
    group group
    mode "0700"
  end
end

node.default["pihole"]["paths"]["root"] = "#{phdir}/conf"
node.default["pihole"]["paths"]["logs"] = "#{phdir}/log"
node.default["pihole"]["container"]["Pod"] = "web.pod"
include_recipe "pihole"

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
