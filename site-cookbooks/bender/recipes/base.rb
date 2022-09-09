file "/etc/apt/apt.conf.d/local" do
  owner "root"
  group "root"
  mode '644'
  content 'Dpkg::Options {
   "--force-confdef";
   "--force-confold";
}'
  action :create
end

apt_update 'update apt cache'

cookbook_file "/etc/apt/sources.list" do
  source "apt.sources.list"
  mode "0644"
  notifies :update, 'apt_update[update apt cache]', :immediately
end

package "base" do
  package_name %w(curl htop iotop iperf)
end

node.override["dnscrypt_proxy"]["listen_address"] = "127.0.2.1"
include_recipe "dnscrypt_proxy"

nethost = node["bender"]["network"]["host"]
addresses = nethost["ipv4"]["addrs"] + nethost["ipv6"]["addrs"]

package "netplan.io" do
  action :upgrade
end

execute "netplan-apply" do
  action :nothing
  command "netplan apply"
end

template "/etc/netplan/01-netcfg.yaml" do
  source "netplan.erb"
  variables(
    interface: node["bender"]["network"]["host"]["interface"],
    addresses: addresses,
    extra_addresses: lazy do
      node["bender"]["firewall"]["ipv6"]["nat"].values.sort.uniq.map { |x| "#{x}/64" }
    end,
    dnscrypt_proxy_address: lazy { node["dnscrypt_proxy"]["listen_address"] },
  )
  notifies :run, "execute[netplan-apply]", :immediately
end
