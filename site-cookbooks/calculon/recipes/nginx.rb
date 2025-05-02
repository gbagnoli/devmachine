calculon_btrfs_volume node["calculon"]["storage"]["paths"]["www"]

node.override["podman"]["nginx"]["path"] = node["calculon"]["storage"]["paths"]["www"]
node.override["podman"]["nginx"]["status"]["enable"] = true
node.override["podman"]["nginx"]["status"]["allow"] = [
  node["calculon"]["network"]["containers"]["ipv4"]["addr"],
  node["calculon"]["network"]["containers"]["ipv6"]["addr"]
]
node.override["podman"]["nginx"]["pod_extra_config"] = %w{
  Network=calculon.network
}

include_recipe "podman_nginx"

calculon_firewalld_port "nginx" do
  port %w{80/tcp 443/tcp}
end
