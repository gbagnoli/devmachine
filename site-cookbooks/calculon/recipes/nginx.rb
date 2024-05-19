calculon_btrfs_volume node["calculon"]["storage"]["paths"]["www"]

node.override["podman"]["nginx"]["pod_extra_conf"] = ["Network=calculon.network"]
node.override["podman"]["nginx"]["path"] = www

include_recipe "podman_nginx"
include_recipe "podman_nginx::acme"

calculon_firewalld_port "nginx" do
  port %w{80/tcp 443/tcp}
end
