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
node.override["datadog"]["enable_process_agent"] = true
node.override["datadog"]["system_probe"]["enabled"] = true
node.override["datadog"]["system_probe"]["network_enabled"] = true
node.override["datadog"]["system_probe"]["service_monitoring_enabled"] = true

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

{
  "filebrowser" => 8385,
  "syncthing" => 8384,
  "magiustaff-filebrowser" => 8387,
  "magiustaff-syncthing" => 8386,
}.each do |name, port|
  node.override["calculon"]["http_checks"][name] = {
    name: name,
    url: "http://#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:#{port}",
  }
end

# monitor ssh for containers
%w(tcp http).each do |type|
  datadog_monitor "#{type}_check" do
    instances(lazy { node["calculon"]["#{type}_checks"].values.sort_by { |c| c[:name] } })
  end
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
