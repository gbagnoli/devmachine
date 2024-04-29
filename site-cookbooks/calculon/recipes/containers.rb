podman_image "syncthing" do
  config(
    Image: ["Image=docker.io/syncthing/syncthing"],
  )
end

sync = node["calculon"]["storage"]["paths"]["sync"]

podman_container "syncthing" do
  config(
    Container: %W{
      Image=syncthing.image
      Environment=PUID=#{node["calculon"]["data"]["uid"]}
      Environment=PGID=#{node["calculon"]["data"]["gid"]}
      PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:8384:8384
      PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:8384:8384
      PublishPort=[::]:22000:22000/tcp
      PublishPort=[::]:22000:22000/udp
      PublishPort=22000:22000/tcp
      PublishPort=22000:22000/udp
      Volume=#{sync}:/var/syncthing
      HostName=sync.tigc.eu
      Network=calculon.network
    },
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Start Syncthing file synchronization",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_firewalld_port "syncthing" do
  port %w{22000/tcp 22000/udp}
end

podman_image "filebrowser" do
  config(
    Image: ["Image=docker.io/filebrowser/filebrowser"],
  )
end

fbdir = "#{sync}/.filebrowser"
fbfile = "#{fbdir}/database.db"
fbsettings = "#{fbdir}/settings.json"

directory fbdir do
  owner node["calculon"]["data"]["username"]
  group node["calculon"]["data"]["group"]
  mode "0700"
end

file fbfile do
  owner node["calculon"]["data"]["username"]
  group node["calculon"]["data"]["group"]
  mode "0700"
  action :create_if_missing
end

require 'json'
file fbsettings do
  owner node["calculon"]["data"]["username"]
  group node["calculon"]["data"]["group"]
  mode "0700"
  content({
    port: 8080,
    baseURL: "/files",
    address: "0.0.0.0",
    log: "stdout",
    database: "/database.db",
    root: "/files"
  }.to_json)
  notifies :restart, "service[filebrowser]", :delayed
end

podman_container "filebrowser" do
  config(
    Container: [
      "Image=filebrowser.image",
      "Environment=PUID=#{node["calculon"]["data"]["uid"]}",
      "Environment=PGID=#{node["calculon"]["data"]["gid"]}",
      "PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:8385:8080",
      "PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:8385:8080",
      "Volume=#{sync}:/files",
      "Volume=#{fbfile}:/database.db",
      "Volume=#{fbsettings}:/.filebrowser.json",
      "Network=calculon.network",
      "HostName=files.tigc.eu",
      "Exec=-a 0.0.0.0 -r /files/ -p 8080 -c /.filebrowser.json",
    ],
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Filebrowser for syncthing data",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

service "filebrowser" do
  action :start
end

calculon_www_upstream "/sync" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8384
  title "Syncthing GUI"
end

calculon_www_upstream "/files" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8385
  title "Syncthing Files"
end
