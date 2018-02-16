# frozen_string_literal: true

apt_repository 'dnscrypt' do
  uri 'ppa:anton+/dnscrypt'
  distribution 'xenial'
end

package 'dnscrypt-proxy'

file '/etc/default/dnscrypt-proxy' do
  content <<~EOH
    DNSCRYPT_PROXY_LOCAL_ADDRESS1=127.0.2.1:53
    DNSCRYPT_PROXY_RESOLVER_NAME1=#{node['dnscrypt_proxy']['resolver']}
    DNSCRYPT_PROXY_OPTIONS=""
  EOH
  notifies :restart, 'service[dnscrypt-proxy]', :delayed
end

service 'dnscrypt-proxy' do
  action %i[enable start]
end

service 'dnscrypt-proxy-resolvconf' do
  action %i[enable start]
end
