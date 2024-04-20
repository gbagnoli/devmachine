tsdir = node["calculon"]["storage"]["paths"]["tailscale"]
user = "nobody"
group = "nobody"
auth_key = node["calculon"]["tailscale"]["authkey"]

calculon_btrfs_volume tsdir do
  owner user
  group group
end

podman_image "tailscale" do
  config(
    Image: ["Image=docker.io/tailscale/tailscale"],
  )
end

podman_container "tailscale" do
  config(
    Container: %W{
      Image=tailscale
      Volume=#{tsdir}:/var/lib/tailscale:rw
      Environment=TS_STATE_DIR=/var/lib/tailscale
      Environment=TS_USERSPACE=0
      Environment=TS_AUTHKEY=#{auth_key}
      Environment=TS_HOSTNAME=calculon.tigc.eu
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
