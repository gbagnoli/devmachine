include_recipe "podman::install"

podman_network "rupik" do
  config(
    Network: %W{
      Driver=bridge
      IPv6=True
      DisableDNS=True
      Subnet=#{node["rupik"]["podman"]["ipv4"]["network"]}
      Subnet=#{node["rupik"]["podman"]["ipv6"]["network"]}
      Gateway=#{node["rupik"]["podman"]["ipv4"]["addr"]}
      Gateway=#{node["rupik"]["podman"]["ipv6"]["addr"]}
    }
  )
end

systemd_unit 'podman-auto-update.timer' do
  action %i{enable start}
end
