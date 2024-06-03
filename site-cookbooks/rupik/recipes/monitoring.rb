if node["datadog"]["api_key"].nil? || node["datadog"]["application_key"].nil?
  Chef::Log.error("skipping monitoring config as no datadog api key or application key are set")
  return
end

include_recipe "datadog::dd-agent"
include_recipe "datadog::dd-handler"

node.override["datadog"]["tags"] = [
  "datacenter:bcn",
  "availiability-zone:ftwo-bcn-a",
  "role:pihole",
  "role:sync",
  "role:vpn",
  "env:home",
  "region:ftwo"
]

file "/etc/datadog-agent/conf.d/btrfs.yaml" do
  action :delete
end

node.override["rupik"]["tcp_checks"]["rupisk_ssh"] = {
  name: "rupik_ssh",
  host: "localhost",
  port: 22,
}

# monitor ssh for containers
datadog_monitor "tcp_check" do
  instances(lazy { node["rupik"]["tcp_checks"].values.sort_by { |c| c[:name] } })
end

include_recipe "datadog::network"

# nginx
node.override["datadog"]["nginx"]["instances"] = [{
  "nginx_status_url" => "http://localhost/nginx_status/",
  "tags" => ["prod"],
}]

include_recipe "datadog::nginx"

# btrfs
node.override["datadog"]["btrfs"]["init_config"]["service"] = nil
node.override["datadog"]["btrfs"]["instances"] = [{}]
include_recipe "datadog::btrfs"

package 'iputils-ping'
datadog_integration "datadog-ping" do
  version "1.0.2"
  third_party true
end

datadog_monitor "ping" do
  use_integration_template true
  instances([{
    # calculon and rupik
    "hosts" => ["100.98.243.29", "100.126.221.76"]
  },{
    "hosts" => ["calculon.tigc.eu"]
  }])
end
