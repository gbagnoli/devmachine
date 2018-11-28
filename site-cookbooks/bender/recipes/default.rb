include_recipe 'lxd::default'
include_recipe 'bender::base'
include_recipe 'bender::users'
include_recipe 'bender::firewall'

lxd_config 'bender' do
  path '/var/lib/lxd.yaml'
  content node['bender']['lxd']['config']
end
