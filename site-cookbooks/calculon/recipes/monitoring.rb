if node["datadog"]["api_key"].nil? || node["datadog"]["application_key"].nil?
  Chef::Log.error("skipping monitoring config as no datadog api key or application key are set")
  return
end

node.override["datadog"]["tags"] = [
  "datacenter:ams",
  "availiability-zone:oneprovider-ams-a",
  "role:host",
  "role:sync",
  "role:media",
  "role:vpn",
  "env:cloud",
  "region:oneprovider"
]

include_recipe "datadog::dd-agent"
include_recipe "datadog::dd-handler"

node.override["calculon"]["tcp_checks"]["calculon_ssh"] = {
  name: "calculon_ssh",
  host: "localhost",
  port: 22,
}

node.override["calculon"]["tcp_checks"]["calculon_et"] = {
  name: "calculon_et",
  host: "localhost",
  port: 2202,
}

# monitor ssh for containers
datadog_monitor "tcp_check" do
  instances(lazy { node["calculon"]["tcp_checks"].values.sort_by { |c| c[:name] } })
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
