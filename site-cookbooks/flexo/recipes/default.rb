node.override['server']['chef']['cron']['minute'] = '45'
include_recipe 'nginx'
include_recipe 'flexo::oauth2_proxy'

include_recipe 'flexo::media'
node.override['plex']['channel'] = 'plexpass' unless node['plex']['token'].nil?
include_recipe 'plex::default'

include_recipe 'datadog::dd-handler'
