emails = Array(node["calculon"]["magiusstaff"]["user_emails"])
domain = node["calculon"]["magiusstaff"]["domain"].to_s

if emails.empty? || domain.empty?
  Chef::Log.info("Skipping magiusstaff as it is not configured in secrets")
end

include_recipe "calculon::base"
include_recipe "calculon::syncthing"
username = node["calculon"]["magiusstaff"]["username"]
groupname = node["calculon"]["magiusstaff"]["group"]
user_uid = node["calculon"]["magiusstaff"]["uid"]
group_gid = node["calculon"]["magiusstaff"]["gid"]
root = node["calculon"]["magiusstaff"]["paths"]["root"]
ipv6 = node["calculon"]["network"]["containers"]["ipv6"]["addr"]
ipv4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]

syncd = "#{root}/sync"

group groupname do
  gid group_gid
end

user username do
  uid user_uid
  gid group_gid
  system true
  shell "/bin/false"
end

directory root

calculon_btrfs_volume syncd do
  group groupname
  owner username
  mode "2775"
  setfacl true
end

podman_container "magiusstaff-syncthing" do
  config(
    Container: %W{
      Image=syncthing.image
      Environment=PUID=#{user_uid}
      Environment=PGID=#{group_gid}
      PublishPort=[#{ipv6}]:8386:8384
      PublishPort=#{ipv4}:8386:8384
      PublishPort=[::]:22200:22000/tcp
      PublishPort=[::]:22200:22000/udp
      PublishPort=22200:22000/tcp
      PublishPort=22200:22000/udp
      Volume=#{syncd}:/var/syncthing
      HostName=magiustaff-sync.tigc.eu
      Network=calculon.network
    },
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Magiusstaff Syncthing file synchronization",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_firewalld_port "magiusstaff-syncthing" do
  port %w{22200/tcp 22200/udp}
end

fbdir = "#{syncd}/.filebrowser"
fbfile = "#{fbdir}/database.db"
fbsettings = "#{fbdir}/settings.json"

directory fbdir do
  owner username
  group groupname
  mode "0700"
end

file fbfile do
  owner username
  group groupname
  mode "0700"
  action :create_if_missing
end

require 'json'
file fbsettings do
  owner username
  group groupname
  mode "0700"
  content({
    port: 8080,
    baseURL: "/files",
    address: "0.0.0.0",
    log: "stdout",
    database: "/database.db",
    root: "/files"
  }.to_json)
  notifies :restart, "service[magiusstaff-filebrowser]", :delayed
end

podman_container "magiusstaff-filebrowser" do
  config(
    Container: [
      "Image=filebrowser.image",
      "Environment=PUID=#{user_uid}",
      "Environment=PGID=#{group_gid}",
      "PublishPort=[#{ipv6}]:8387:8080",
      "PublishPort=#{ipv4}:8387:8080",
      "Volume=#{syncd}:/files",
      "Volume=#{fbfile}:/database.db",
      "Volume=#{fbsettings}:/.filebrowser.json",
      "Network=calculon.network",
      "HostName=magiusstaff-files.tigc.eu",
      "User=#{user_uid}:#{group_gid}",
      "Exec=-a 0.0.0.0 -r /files/ -p 8080 -c /.filebrowser.json",
    ],
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=Magiusstaff Filebrowser for syncthing data",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

service "magiusstaff-filebrowser" do
  action :start
end

upstreams = {
    "/sync" => {
      "upstream" => "http://[#{ipv6}]:8386",
      "title" => "Syncthing GUI",
      "extra_properties" => {
        proxy_read_timeout: "600s",
        proxy_send_timeout: "600s",
      },
      "category" => "Files"
    },
    "/files" => {
      "upstream" => "http://[#{ipv6}]:8387",
      "title" => "Browse Files",
      "extra_properties" => {
        client_max_body_size: "2048m",
        proxy_read_timeout: "86400s",
        proxy_send_timeout: "86400s",
      },
      "category" => "Files"
    }
}

calculon_vhost domain do
  server_name domain
  cloudflare true
  upstream_paths upstreams
  oauth2_proxy(
    emails: emails,
    port: 4102
  )
  act_as_upstream 4103
end

www = node["calculon"]["storage"]["paths"]["www"]
template "#{www}/vhosts/#{domain}/index.html" do
  source "www_host_index.erb"
  variables(
    upstreams: upstreams,
    domain: domain,
  )
end
