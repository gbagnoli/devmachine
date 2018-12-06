node.override['server']['chef']['cron']['minute'] = '45'
include_recipe 'bender::users'
include_recipe 'nginx-hardening::upgrades'
include_recipe 'nginx'
include_recipe 'nginx-hardening'

include_recipe 'marvin::oauth2_proxy'
include_recipe 'flexo::media'
node.override['plex']['channel'] = 'plexpass' unless node['plex']['token'].nil?
include_recipe 'plex::default'
