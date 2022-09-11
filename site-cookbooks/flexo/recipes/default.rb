node.override["server"]["chef"]["cron"]["minute"] = "45"
nginx_install 'nginx' do
  source 'repo'
end

nginx_service 'nginx' do
  action :enable
  delayed_action :start
end
include_recipe "flexo::oauth2_proxy"

include_recipe "flexo::media"
node.override["plex"]["channel"] = "plexpass" unless node["plex"]["token"].nil?
include_recipe "plex::default"
