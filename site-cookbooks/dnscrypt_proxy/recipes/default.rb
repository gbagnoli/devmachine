if node['platform'] == 'ubuntu'
  include_recipe 'dnscrypt_proxy::ubuntu'
else
  include_recipe 'dnscrypt_proxy::autoinstall'
end
