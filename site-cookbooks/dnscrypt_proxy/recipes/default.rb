git node['dnscrypt_proxy']['src_dir'] do
  repository node['dnscrypt_proxy']['repository']
  revision node['dnscrypt_proxy']['revision']
  action :checkout
  notifies :run, 'execute[install dnscrypt_proxy]', :immediately
end

execute 'install dnscrypt_proxy' do
  action :nothing
  cwd node['dnscrypt_proxy']['src_dir']
  command 'chmod +x dnscrypt-autoinstall && yes | ./dnscrypt-autoinstall'
end
