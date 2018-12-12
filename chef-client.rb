# rubocop:disable Naming/FileName
root = File.dirname(__FILE__)
chef_repo_path root
chef_zero_enabled true
cookbook_path ["#{root}/berks-cookbooks"]
data_bag_path "#{root}/data-bags"
environment 'default'
environment_path "#{root}/environments"
node_path "#{root}/nodes"
role_path "#{root}/roles"
ohai.plugin_path << "#{root}/ohai/plugins"
# rubocop:enable Naming/FileName
