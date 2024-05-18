if node.platform_family? "debian"
  include_recipe "apt::unattended-upgrades"
  user "ubuntu" do
    action :remove
  end
else
  # fedora needs crontab
  package 'cronie'
end

include_recipe "chef_client_updater" if node["server"]["components"]["chef_client_updater"]["enabled"]
include_recipe "hardening"
include_recipe "user" unless node["server"]["components"]["user"]["enabled"] == false
include_recipe "server::users"

unless node["server"]["components"]["chef_client_cron"]["enabled"] == false
  include_recipe "server::chef_client_cron"
end
