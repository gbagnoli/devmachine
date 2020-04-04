if node["datadog"]["api_key"].nil? || node["datadog"]["application_key"].nil?
  Chef::Log.error("Not adding datadog handler as no api key or application key are set")
else
  include_recipe "datadog::dd-handler"
end
