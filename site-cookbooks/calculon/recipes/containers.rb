include_recipe "podman::install"

path = node["calculon"]["containers"]["storage"]["volume"]

execute "create subvolume at #{path}" do
  command "btrfs subvolume create #{path}"
  not_if "btrfs subvolume show #{path} &>/dev/null"
end

template "/etc/containers/storage.conf" do
  variables node["calculon"]["containers"]["storage"]
  source "podman_storage.conf.erb"
  owner "root"
  group "root"
  mode "0755"
  notifies :run, "execute[podman_system_reset]", :before
end

podman_network "calculon" do
  config(
    Network: [
      "Driver=bridge",
      "IPv6=True",
      "Subnet=#{node["calculon"]["network"]["containers"]["ipv4"]["network"]}",
      "Subnet=#{node["calculon"]["network"]["containers"]["ipv6"]["network"]}",
      "Gateway=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}",
      "Gateway=#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}",
    ]
  )
end

podman_image "syncthing" do
  config(
    Image: ["Image=docker.io/syncthing/syncthing"],
  )
end

podman_container "syncthing" do
  config(
    Container: [
      "Image=syncthing.image",
      "Environment=PUID=#{node["calculon"]["data"]["uid"]}",
      "Environment=PGID=#{node["calculon"]["data"]["gid"]}",
      "PublishPort=8384:8384",
      "PublishPort=22000:22000/tcp",
      "PublishPort=22000:22000/udp",
      "Volume=#{node["calculon"]["storage"]["paths"]["sync"]}:/var/syncthing",
      "HostName=sync.tigc.eu",
      "Network=calculon.network",
    ],
    Service: [
      "Restart=Always"
    ],
    Unit: [
      "Description=Start Syncthing file synchronization",
    ],
    Install: [
      "WantedBy=multi-user.target"
    ]
  )
end
