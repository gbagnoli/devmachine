conf = node["tailscale"]["podman"]
auth_key = node["tailscale"]["authkey"]
hostname = conf["hostname"] || node["fqdn"]

package "ethtool"

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

podman_image "tailscale" do
  config(
    Image: ["Image=docker.io/tailscale/tailscale"],
  )
end

directory conf["config_dir"] do
  owner conf["user"]
  group conf["group"]
  mode "0700"
end

extra_env = conf["extra_env"].to_h.map do |k, v|
  "Environment=#{k.upcase}=#{v}"
end

extra_env << "Environment=TS_AUTHKEY=#{auth_key}" unless auth_key.nil?

podman_container "tailscale" do
  config(
    Container: %W{
      Network=host
      Image=tailscale.image
      Volume=#{conf["config_dir"]}:/var/lib/tailscale:rw
      Environment=PORT=#{node["tailscale"]["port"]}
      Environment=TS_STATE_DIR=/var/lib/tailscale
      Environment=TS_USERSPACE=0
      Environment=TS_HOSTNAME=#{hostname}
      AddDevice=/dev/net/tun:/dev/net/tun:rw
      AddCapability=NET_ADMIN
      AddCapability=NET_RAW
    } + extra_env,
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Start tailscaled",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end

service "tailscaled" do
  service_name "tailscale"
end
