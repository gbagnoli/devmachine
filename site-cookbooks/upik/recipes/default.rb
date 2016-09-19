package 'btrfs-progs'
package 'dstat'
package 'exfat-fuse'
package 'exfat-utils'
package 'htop'
package 'openvpn'
package 'shellcheck'
package 'tmux'
package 'zfs-dkms'
package 'zfsutils-linux'
package 'zfs-initramfs'

file '/etc/iptables.rules' do
  content <<-EOH
*nat
:PREROUTING ACCEPT [0:0]
:INPUT ACCEPT [0:0]
:OUTPUT ACCEPT [0:0]
:POSTROUTING ACCEPT [0:0]
-A POSTROUTING -s 192.168.1.0/24 -d 172.31.90.0/24 -j MASQUERADE
COMMIT
  EOH
end

file '/usr/local/bin/reload_iptables_rules' do
  mode '0755'
  content <<-EOH
#!/bin/bash
/bin/echo 1 > /proc/sys/net/ipv4/ip_forward
/sbin/iptables-restore < /etc/iptables.rules
/sbin/iptables -t nat -Z
  EOH
end

directory '/srv/snapshots/sync' do
  recursive true
end

file '/etc/btrbk/btrbk.conf' do
  mode '0644'
  content <<-EOH
timestamp_format        long
snapshot_preserve_min   6h
snapshot_preserve       24h 31d 6m

volume /srv
  snapshot_dir snapshots/sync
  subvolume sync
  EOH
end

file '/etc/cron.hourly/btrbk' do
  content <<-EOH
#!/bin/sh
exec /usr/sbin/btrbk -q run
  EOH
  mode '0755'
end
