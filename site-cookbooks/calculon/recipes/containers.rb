include_recipe "podman::install"

path = node["calculon"]["containers"]["storage"]["volume"]

execute "create subvolume at #{path}" do
  command "btrfs subvolume create #{path}"
  not_if "btrfs subvolume show #{path} &>/dev/null"
end

template "/etc/containers/storage.conf" do
  variables node["calculon"]["containers"]["storage"]
  source "podman_storage.conf.erb"
  owner "root"
  group "root"
  mode "0755"
  notifies :run, "execute[podman_system_reset]", :before
end

podman_network "calculon" do
  config(
    Network: [
      "Driver=Bridge",
      "IPv6=True",
      "Subnet=#{node["calculon"]["network"]["containers"]["ipv4"]["network"]}",
      "Subnet=#{node["calculon"]["network"]["containers"]["ipv6"]["network"]}",
      "Gateway=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}",
      "Gateway=#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}",
    ]
  )
  action %i{create start}
end
