include_recipe 'apt'
include_recipe 'apt::unattended-upgrades'
include_recipe 'chef_client_updater'
include_recipe 'hardening'
include_recipe 'user' unless node['server']['components']['user']['enabled'] == false
include_recipe 'server::users'
include_recipe 'syncthing' unless node['server']['components']['syncthing']['enabled'] == false
include_recipe 'server::chef_client_cron' \
  unless node['server']['components']['chef_client_cron']['enabled'] == false

user 'ubuntu' do
  action :remove
end
