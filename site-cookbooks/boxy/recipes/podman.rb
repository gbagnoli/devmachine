include_recipe "podman::install"

podman_network "boxy" do
  config(
    Network: %W{
      Driver=bridge
      IPv6=True
      DisableDNS=True
      Subnet=#{node["boxy"]["podman"]["ipv4"]["network"]}
      Subnet=#{node["boxy"]["podman"]["ipv6"]["network"]}
      Gateway=#{node["boxy"]["podman"]["ipv4"]["addr"]}
      Gateway=#{node["boxy"]["podman"]["ipv6"]["addr"]}
    }
  )
end

systemd_unit 'podman-auto-update.timer' do
  action %i{enable start}
end
