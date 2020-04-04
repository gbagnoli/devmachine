if node["datadog"]["api_key"].nil? || node["datadog"]["application_key"].nil?
  Chef::Log.error("skipping monitoring config as no datadog api key or application key are set")
  return
end

include_recipe "datadog::dd-agent"
include_recipe "datadog::dd-handler"

include_recipe "nginx::http_stub_status_module"

node.override["datadog"]["nginx"]["instances"] = [{
  "nginx_status_url" => "http://localhost:#{node["nginx"]["status"]["port"]}/nginx_status/",
  "tags" => %w[bender prod],
}]

include_recipe "datadog::nginx"

package "dd-check-btrfs"
directory "/etc/dd-agent/conf.d" do
  owner "dd-agent"
  group "dd-agent"
  mode "0755"
end

file "/etc/dd-agent/conf.d/btrfs.yaml" do
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

node.override["bender"]["tcp_checks"]["bender_ssh"] = {
  name: "bender_ssh",
  host: "localhost",
  port: 22,
}

# monitor ssh for containers
datadog_monitor "tcp_check" do
  instances(lazy { node["bender"]["tcp_checks"].values.sort_by { |c| c[:name] } })
end
