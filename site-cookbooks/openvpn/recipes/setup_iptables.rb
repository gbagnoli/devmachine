cookbook_file "/usr/local/bin/openvpn-setup-iptables" do
  source "openvpn-setup-iptables"
  action :create
  mode "0750"
end

systemd_unit "openvpn-setup-iptables.service" do
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
  action %i(create enable)
end
