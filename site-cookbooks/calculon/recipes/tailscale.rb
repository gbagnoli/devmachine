tsdir = node["calculon"]["storage"]["paths"]["tailscale"]
user = "nobody"
group = "nobody"
auth_key = node["calculon"]["tailscale"]["authkey"]

calculon_btrfs_volume tsdir do
  owner user
  group group
  mode "0700"
end

sysctl "net.ipv6.conf.all.forwarding" do
  value "1"
end

sysctl "net.ipv4.ip_forward" do
  value "1"
end

execute "enable UDP offload" do
  command "ethtool -K eth0 rx-udp-gro-forwarding on rx-gro-list off"
  not_if "ethtool -k eth0 | grep -q 'rx-udp-gro-forwarding: on'"
end

bash "enable masquerading" do
  code <<~EOH
    firewall-cmd --add-masquerade --zone=public
    firewall-cmd --permanent --add-masquerade --zone=public
  EOH
  not_if "firewall-cmd --query-masquerade --zone=public --permanent | grep -q yes"
end


podman_image "tailscale" do
  config(
    Image: ["Image=docker.io/tailscale/tailscale"],
  )
end

podman_container "tailscale" do
  config(
    Container: %W{
      Network=host
      Image=tailscale.image
      Volume=#{tsdir}:/var/lib/tailscale:rw
      Environment=PORT=39129
      Environment=TS_STATE_DIR=/var/lib/tailscale
      Environment=TS_USERSPACE=0
      Environment=TS_AUTHKEY=#{auth_key}
      Environment=TS_HOSTNAME=calculon.tigc.eu
      Environment=TS_ROUTES=#{node["calculon"]["network"]["containers"]["ipv4"]["network"]},#{node["calculon"]["network"]["containers"]["ipv6"]["network"]}
      Environment=TS_EXTRA_ARGS=--advertise-exit-node
      AddDevice=/dev/net/tun:/dev/net/tun:rw
      AddCapability=NET_ADMIN
      AddCapability=NET_RAW
    },
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Start tailscaled",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_firewalld_port "tailscaled" do
  port %w{39129/udp}
end
