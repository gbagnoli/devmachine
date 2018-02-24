include_recipe 'openvpn'

node.override['dnscrypt_proxy']['bind_address'] = '0.0.0.0'
include_recipe 'dnscrypt_proxy'

node.override['nginx']['default_site_enabled'] = false
include_recipe 'nginx::default'

include_recipe 'marvin::oauth2_proxy'
include_recipe 'marvin::thelounge'
