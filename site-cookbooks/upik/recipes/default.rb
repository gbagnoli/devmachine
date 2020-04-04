# frozen_string_literal: true

include_recipe "upik::apt"

package "curl"
package "dstat"
package "exfat-fuse"
package "exfat-utils"
package "htop"
package "jq"
package "openvpn"
package "shellcheck"
package "tmux"
package "vim.nox"

file "/etc/network/interfaces" do
  content <<~EOC
            # Managed by Chef
            source /etc/network/interfaces.d/*

            # The loopback network interface
            auto lo
            iface lo inet loopback

            allow-hotplug eth0
          EOC
  mode "0644"
end

directory "/etc/network/interfaces.d"

template "/etc/network/interfaces.d/eth0" do
  source "iface.erb"
  variables(
    iface: "eth0",
    ipv6: node["upik"]["ipv6"],
    address: node["upik"]["address"],
    netmask: node["upik"]["netmask"],
    gateway: node["upik"]["gateway"],
    dns_nameservers: node["upik"]["dns-nameservers"],
  )
  mode "0644"
end

service "wicd" do
  action %i[stop disable]
end

# neovim pinning
apt_preference "libmsgpackc2" do
  pin "release n=unstable"
  pin_priority "900"
end

execute "install libmsgpackc2" do
  command "apt-get install libmsgpackc2/unstable"
  not_if 'dpkg -l libmsgpackc2 | grep "^ii" -q'
end

lnet = node["upik"]["local_network"]
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

directory "/srv/snapshots/sync" do
  recursive true
end

directory "/etc/btrbk" do
  mode "0755"
end

file "/etc/btrbk/btrbk.conf" do
  mode "0644"
  content <<~EOH
            timestamp_format        long
            snapshot_preserve_min   6h
            snapshot_preserve       24h 31d 6m

            volume /srv
              snapshot_dir snapshots/sync
              subvolume sync
          EOH
end

file "/etc/cron.hourly/btrbk" do
  content <<~EOH
            #!/bin/sh
            exec /usr/sbin/btrbk -q run
          EOH
  mode "0755"
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

include_recipe "upik::unifi"
