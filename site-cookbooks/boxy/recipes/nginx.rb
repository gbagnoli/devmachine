node.override["podman"]["nginx"]["pod_extra_config"] = %w{
  PublishPort=[::]:53:53/tcp
  PublishPort=53:53/tcp
  PublishPort=[::]:53:53/udp
  PublishPort=53:53/udp
}

node.override["podman"]["nginx"]["default_vhost"]["template"] = "default_vhost.erb"
node.override["podman"]["nginx"]["default_vhost"]["cookbook"] = "boxy"
node.override["podman"]["nginx"]["acme"]["lego"]["provider"] = "cloudflare"
node.override["podman"]["nginx"]["acme"]["lego"]["environment"] = {
  "CF_DNS_API_TOKEN" => node["cloudflare"]["dns_api_token"],
  "CF_ZONE_API_TOKEN" => node["cloudflare"]["zone_api_token"],
}

node.override["podman"]["nginx"]["status"]["enable"] = true
node.override["podman"]["nginx"]["status"]["allow"] = [
  "10.88.0.1"
]

include_recipe "podman_nginx"
include_recipe "podman_nginx::oauth2_proxy"
