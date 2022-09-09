include_recipe "apt::unattended-upgrades"
include_recipe "chef_client_updater" if node["server"]["components"]["chef_client_updater"]["enabled"]
include_recipe "hardening"
include_recipe "user" unless node["server"]["components"]["user"]["enabled"] == false
include_recipe "server::users"
include_recipe "syncthing" unless node["server"]["components"]["syncthing"]["enabled"] == false
unless node["server"]["components"]["chef_client_cron"]["enabled"] == false
  include_recipe "server::chef_client_cron"
end

user "ubuntu" do
  action :remove
end
