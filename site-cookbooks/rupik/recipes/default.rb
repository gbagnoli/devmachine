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

node.override["podman"]["nginx"]["default_vhost"]["template"] = "default_vhost.erb"
node.override["podman"]["nginx"]["default_vhost"]["cookbook"] = "rupik"
node.override["podman"]["nginx"]["acme"]["lego"]["provider"] = "cloudflare"
node.override["podman"]["nginx"]["acme"]["lego"]["environment"] = {
  "CF_DNS_API_TOKEN" => node["cloudflare"]["dns_api_token"],
  "CF_ZONE_API_TOKEN" => node["cloudflare"]["zone_api_token"],
}

include_recipe "podman_nginx"
include_recipe "podman_nginx::acme"

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
