include_recipe 'openvpn'

file '/usr/local/bin/openvpn-setup-iptables' do
  action :create
  content <<~EOC
    #!/bin/bash
    iptables -t nat -A POSTROUTING -s 172.31.0.0/16 -o eth0 -j MASQUERADE
    iptables -t nat -A PREROUTING -s 172.31.0.0/16 -p udp --dport 53 -j DNAT --to 172.31.90.1:54
    iptables -t nat -A PREROUTING -s 172.31.0.0/16 -p tcp --dport 53 -j DNAT --to 172.31.90.1:54
    iptables -A FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
  EOC
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
  action %i[create enable start]
end
