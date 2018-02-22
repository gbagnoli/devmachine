include_recipe 'marvin::oauth2_proxy'
include_recipe 'marvin::thelounge'

node.override['nginx']['default_site_enabled'] = false
include_recipe 'nginx::default'
