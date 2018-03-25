include_recipe 'marvin::openvpn'

node.override['dnscrypt_proxy']['bind_address'] = '0.0.0.0'
include_recipe 'dnscrypt_proxy'

if node['lsb']['codename'] == 'bionic'
  # bionic still doesn't have packages in the PPA
  node.override['nginx']['repo_source'] = 'distro'
end
node.override['nginx']['default_site_enabled'] = false
include_recipe 'nginx::default'

include_recipe 'marvin::oauth2_proxy'
include_recipe 'marvin::thelounge'

include_recipe 'marvin::media'

node.override['plex']['channel'] = 'plexpass' unless node['plex']['token'].nil?
include_recipe 'plex::default'
