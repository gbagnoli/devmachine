# frozen_string_literal: true

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

service 'dnscrypt-autoinstall' do
  action %i[enable start]
end

service 'dnscrypt-autoinstall-backup' do
  action %i[enable start]
end

file '/etc/systemd/system/dnscrypt-autoinstall.conf' do
  content <<~EOH
    DNSCRYPT_LOCALIP=127.0.0.1
    DNSCRYPT_LOCALIP2=127.0.0.2
    DNSCRYPT_LOCALPORT=53
    DNSCRYPT_LOCALPORT2=53
    DNSCRYPT_USER=dnscrypt
    DNSCRYPT_RESOLVER=#{node['dnscrypt_proxy']['resolver']}
    DNSCRYPT_RESOLVER2=#{node['dnscrypt_proxy']['resolver']}
EOH
  notifies :restart, 'service[dnscrypt-autoinstall]', :delayed
  notifies :restart, 'service[dnscrypt-autoinstall-backup]', :delayed
end
