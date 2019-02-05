include_recipe 'openvpn'
include_recipe 'openvpn::openvpn-setup-iptables'
cookbook_file '/usr/local/bin/openvpn-setup-iptables' do
  source 'openvpn-setup-iptables'
  action :create
  mode '0750'
end

systemd_unit 'openvpn-setup-iptables.service' do
  content <<~EOU
    [Unit]
    Description=Install iptables rules for openvpn

    [Service]
    Type=oneshot
    ExecStart=/usr/local/bin/openvpn-setup-iptables
    After=openvpn.service

    [Install]
    WantedBy=multi-user.target
  EOU
  action %i[create enable]
end

execute 'remove nproclimit from openvpn unit' do
  command 'sed /^LimitNPROC.*$/d /lib/systemd/system/openvpn@.service > /etc/systemd/system/openvpn@.service'
  not_if { ::File.exist?('/etc/systemd/system/openvpn@.service') }
  notifies :run, 'execute[reload-systemd-openvpn]', :immediately
end

execute 'reload-systemd-openvpn' do
  action :nothing
  command 'systemctl daemon-reload'
end
