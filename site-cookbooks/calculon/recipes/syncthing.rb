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
      "User=#{node["calculon"]["data"]["uid"]}:#{node["calculon"]["data"]["gid"]}",
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
  extra_properties(
    proxy_read_timeout: "600s",
    proxy_send_timeout: "600s",
  )
  title "Syncthing GUI"
  category "Tools"
end

calculon_www_upstream "/files" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8385
  upgrade true
  title "Syncthing Files"
  extra_properties(
    client_max_body_size: "2048m",
    proxy_read_timeout: "86400s",
    proxy_send_timeout: "86400s",
  )
  category "Media"
end

include_recipe "btrbk"

volumes = node["calculon"]["storage"]["snapshots_volumes"]
snapd = "#{node["calculon"]["storage"]["paths"]["root"]}/snapshots/"
directory snapd
volumes.each do |vol|
  directory "#{snapd}/#{vol}"
end

directory "/etc/btrbk" do
  mode "0755"
end

template "/etc/btrbk/btrbk.conf" do
  mode "0644"
  source "btrbk.conf.erb"
  variables(
    vol: node["calculon"]["storage"]["paths"]["root"],
    snapshotd: "snapshots",
    subvolumes: node["calculon"]["storage"]["snapshots_volumes"]
  )
end

# disable builtin daily timer
systemd_unit 'btrbk.timer' do
  action :disable
end

systemd_unit 'btrbk_hourly.timer' do
  content <<~EOH
    [Unit]
    Description=btrbk hourly backup

    [Timer]
    OnCalendar=hourly
    Persistent=true
    Unit=btrbk.service

    [Install]
    WantedBy=timers.target
  EOH
  action %i{create enable start}
end

ruby_block "get gpth latest version" do
  block do
    uri = URI("https://api.github.com/repos/TheLastGimbus/GooglePhotosTakeoutHelper/releases/latest")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)
    asset = parsed["assets"].select {|x| x["name"].include?("gpth-linux")}.first
    node.run_state["gpth_download_url"] = asset["browser_download_url"]
    node.run_state["gpth_version"] = parsed["tag_name"][1..]
  end
end

remote_file "/usr/local/bin/gpth" do
  source(lazy { node.run_state["gpth_download_url"] })
  owner "root"
  mode "0755"
end

package "backup_tools" do
  package_name %w{rsync tar parallel}
end

user = node["user"]["login"]
group = node["user"]["group"]
home = "/home/#{user}"

directory "#{home}/.config/rclone" do
  owner user
  group group
  mode "0700"
end

template "#{home}/.config/rclone/rclone.conf" do
  owner user
  group group
  mode "0600"
  variables(remotes: node["rclone"]["remotes"])
end
