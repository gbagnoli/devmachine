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

node.override["podman"]["nginx"]["pod_extra_config"] = %w{
  PublishPort=[::]:53:53/tcp
  PublishPort=53:53/tcp
  PublishPort=[::]:53:53/udp
  PublishPort=53:53/udp
}

include_recipe "podman_nginx"

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
  "TS_ROUTES" => node["rupik"]["lan"]["ipv4"]["network"],
  "TS_EXTRA_ARGS" => "--advertise-exit-node"
}
include_recipe "tailscale::install"
