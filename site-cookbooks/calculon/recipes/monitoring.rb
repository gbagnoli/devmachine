if node["datadog"]["api_key"].nil? || node["datadog"]["application_key"].nil?
  Chef::Log.error("skipping monitoring config as no datadog api key or application key are set")
  return
end

include_recipe "datadog::dd-agent"
include_recipe "datadog::dd-handler"

directory "/etc/datadog-agent/conf.d" do
  owner "dd-agent"
  group "dd-agent"
  mode "0755"
end

file "/etc/datadog-agent/conf.d/btrfs.yaml" do
  content <<~CONTENT
    init_config:
    # Not required for this check

    instances:
      - excluded_devices: []
  CONTENT
  notifies :restart, "service[datadog-agent]"
  owner "dd-agent"
  group "dd-agent"
end

node.override["calculon"]["tcp_checks"]["calculon_ssh"] = {
  name: "calculon_ssh",
  host: "localhost",
  port: 22,
}

node.override["calculon"]["tcp_checks"]["calculon_et"] = {
  name: "calculon_ssh",
  host: "localhost",
  port: 2202,
}

# monitor ssh for containers
datadog_monitor "tcp_check" do
  instances(lazy { node["calculon"]["tcp_checks"].values.sort_by { |c| c[:name] } })
end
