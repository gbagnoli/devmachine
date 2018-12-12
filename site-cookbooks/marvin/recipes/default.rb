include_recipe 'marvin::openvpn'

node.override['syncthing']['users']['giacomo']['hostname'] = 'syncthing.tigc.eu'
node.override['syncthing']['users']['giacomo']['port'] = '8384'
node.override['server']['chef']['cron']['minute'] = '10'
node.override['dnscrypt_proxy']['listen_port'] = '54'
include_recipe 'dnscrypt_proxy'

include_recipe 'nginx-hardening::upgrades'
include_recipe 'nginx'
include_recipe 'nginx-hardening'

include_recipe 'marvin::oauth2_proxy'
include_recipe 'marvin::thelounge'

include_recipe 'datadog::dd-handler'
