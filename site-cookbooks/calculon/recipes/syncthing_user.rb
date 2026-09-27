
username = node["user"]["login"]
container = "#{username}-syncthing"
syncdir = "#{node["user"]["homedir"]}/#{username}/.local/syncthing"
groupname = node["user"]["group"]
user_uid = node["user"]["uid"]
group_gid = node["user"]["gid"]
ipv6 = node["calculon"]["network"]["containers"]["ipv6"]["addr"]
ipv4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]
port = "8388"
external_port = "22202"

directory syncdir do
  recursive true
  owner username
  group groupname
  mode "0700"
end

podman_container container do
  config(
    Container: %W{
      Image=docker.io/syncthing/syncthing:latest
      Environment=PUID=#{user_uid}
      Environment=PGID=#{group_gid}
      PublishPort=[#{ipv6}]:#{port}:8384
      PublishPort=#{ipv4}:#{port}:8384
      PublishPort=[::]:#{external_port}:22000/tcp
      PublishPort=[0.0.0.0]:#{external_port}:22000/tcp
      PublishPort=[::]:#{external_port}:22000/udp
      PublishPort=[0.0.0.0]:#{external_port}:22000/udp
      Volume=#{syncdir}:/var/syncthing
      HostName=#{username}-sync.tigc.eu
      Network=calculon.network
    },
    Service: %w{
      Restart=always
    },
    # description has spaces, use a normal list
    Unit: [
      "Description=#{username} Syncthing file synchronization",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

calculon_firewalld_port "user-syncthing" do
  port(%W{#{external_port}/tcp #{external_port}/udp})
end


calculon_www_upstream "/sync-#{username}" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port port
  extra_properties [
    "proxy_read_timeout 600s",
    "proxy_send_timeout 600s",
  ]
  title "Syncthing GUI (#{username})"
  category "Tools"
end
