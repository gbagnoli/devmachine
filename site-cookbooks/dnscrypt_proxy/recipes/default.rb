# frozen_string_literal: true

include_recipe 'dnscrypt_proxy::cleanup'

srcfile = '/usr/src/dnscrypt_proxy.tar.gz'
tmpdir = '/tmp/dnscrypt-proxy'
arch = node['dnscrypt_proxy']['arch']

remote_file srcfile do
  repo = node['dnscrypt_proxy']['repository_url']
  v = node['dnscrypt_proxy']['version']
  source "#{repo}releases/download/#{v}/dnscrypt-proxy-#{arch}-#{v}.tar.gz"
  notifies :run, 'execute[unpack-dnscrypt_proxy]', :immediately
end

execute 'unpack-dnscrypt_proxy' do
  action :nothing
  command "mkdir -p #{tmpdir} && tar xzf #{srcfile} -C #{tmpdir}"
  notifies :run, 'execute[install-dnscrypt-proxy]', :immediately
end

user 'dnscrypt' do
  system true
  shell '/bin/false'
  group 'nogroup'
end

execute 'install-dnscrypt-proxy' do
  action :nothing
  cwd "#{tmpdir}/#{arch.sub('_', '-')}"
  command 'install -m 755 dnscrypt-proxy /usr/bin/'
end

execute 'remove_capabilities' do
  command 'setcap -r /usr/bin/dnscrypt-proxy'
  only_if 'getcap /usr/bin/dnscrypt-proxy | grep -q /usr/bin/dnscrypt-proxy'
end

directory tmpdir do
  action :delete
  recursive true
end

config = '/etc/dnscrypt-proxy.toml'

template config do
  source 'dnscrypt-proxy.toml.erb'
  owner 'dnscrypt'
  group 'nogroup'
  mode '0444'
end

systemd_unit 'dnscrypt-proxy.service' do
  verify false
  content <<~EOU
    [Unit]
    Description=DNSCrypt client proxy
    Documentation=man:dnscrypt-proxy(8)
    Requires=dnscrypt-proxy.socket
    After=network.target
    Before=nss-lookup.target
    Wants=nss-lookup.target

    [Install]
    Also=dnscrypt-proxy.socket
    WantedBy=multi-user.target

    [Service]
    Type=simple
    NonBlocking=true
    User=dnscrypt
    Group=nogroup
    ExecStart=/usr/bin/dnscrypt-proxy -config #{config}
  EOU
  action %i[create]
end

systemd_unit 'dnscrypt-proxy.socket' do
  verify false
  content <<~EOU
    [Unit]
    Description=dnscrypt-proxy listening socket

    [Socket]
    ListenStream=#{node['dnscrypt_proxy']['listen_address']}:53
    ListenDatagram=#{node['dnscrypt_proxy']['listen_address']}:53
    NoDelay=true
    DeferAcceptSec=1

    [Install]
    WantedBy=sockets.target
  EOU
  action %i[create enable]
  notifies :start, 'systemd_unit[dnscrypt-proxy.service]', :immediately
  notifies :enable, 'systemd_unit[dnscrypt-proxy.service]', :immediately
end
