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
    Network: %W{
      Driver=bridge
      IPv6=True
      Subnet=#{node["calculon"]["network"]["containers"]["ipv4"]["network"]}
      Subnet=#{node["calculon"]["network"]["containers"]["ipv6"]["network"]}
      Gateway=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}
      Gateway=#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}
    }
  )
end

podman_image "syncthing" do
  config(
    Image: ["Image=docker.io/syncthing/syncthing"],
  )
end

podman_container "syncthing" do
  config(
    Container: %W{
      Image=syncthing.image
      Environment=PUID=#{node["calculon"]["data"]["uid"]}
      Environment=PGID=#{node["calculon"]["data"]["gid"]}
      PublishPort=[::1]:8384:8384
      PublishPort=[::]:22000:22000/tcp
      PublishPort=[::]:22000:22000/udp
      PublishPort=22000:22000/tcp
      PublishPort=22000:22000/udp
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}:/var/syncthing
      HostName=sync.tigc.eu
      Network=calculon.network
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Start Syncthing file synchronization",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.targe
    }
  )
end

calculon_firewalld_port "syncthing" do
  port %w{20022/tcp 20022/udp}
end
