# file "/etc/network/interfaces" do
#   content <<~EOC
#     # Managed by Chef
#     source /etc/network/interfaces.d/*

#     # The loopback network interface
#     auto lo
#     iface lo inet loopback

#     allow-hotplug eth0
#   EOC
#   mode "0644"
# end

# directory "/etc/network/interfaces.d"

# template "/etc/network/interfaces.d/eth0" do
#   source "iface.erb"
#   variables(
#     iface: "eth0",
#     ipv6: node["rupik"]["ipv6"],
#     address: node["rupik"]["address"],
#     netmask: node["rupik"]["netmask"],
#     gateway: node["rupik"]["gateway"],
#     dns_nameservers: node["rupik"]["dns-nameservers"],
#   )
#   mode "0644"
# end
#
lnet = node["rupik"]["local_network"]
file "/etc/iptables.rules" do
  content <<~EOH
    *nat
    :PREROUTING ACCEPT [0:0]
    :INPUT ACCEPT [0:0]
    :OUTPUT ACCEPT [0:0]
    :POSTROUTING ACCEPT [0:0]
    -A POSTROUTING -s #{lnet} -d 172.31.90.0/24 -j MASQUERADE
    -A POSTROUTING -s #{lnet} -d 10.0.3.0/24 -j MASQUERADE
    COMMIT
  EOH
end

file "/usr/local/bin/reload_iptables_rules" do
  mode "0755"
  content <<~EOH
    #!/bin/bash
    /bin/echo 1 > /proc/sys/net/ipv4/ip_forward
    /sbin/iptables-restore < /etc/iptables.rules
    /sbin/iptables -t nat -Z
  EOH
end

cookbook_file "/usr/bin/update_cloudflare_ip" do
  source "update_cloudflare_ip"
  mode "0750"
end

cron "update cloudflare ip" do
  command "/usr/bin/update_cloudflare_ip &> /var/log/update_cloudflare_ip.log"
  minute "*/15"
  user "root"
end
