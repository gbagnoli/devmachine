podman_image "tdarr" do
  config(
    Image: ["Image=ghcr.io/haveagitgat/tdarr"],
  )
end


tdarr_root = node["calculon"]["storage"]["paths"]["tdarr"]
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
gid = node["calculon"]["data"]["gid"]

calculon_btrfs_volume tdarr_root do
  owner user
  group group
  mode "2775"
  setfacl true
end

%w[server configs logs cache cache/series cache/movies].each do |dir|
  directory "#{tdarr_root}/#{dir}" do
    owner user
    group group
    mode "2755"
  end
end

podman_container "tdarr" do
  config(
    Container: %W{
      Image=tdarr.image
      Network=calculon.network
      Environment=TZ=Europe/Madrid
      Environment=PUID=#{uid}
      Environment=GUID=#{gid}
      Environment=serverPort=8266
      Environment=webUIPort=8265
      Environment=internalNode=true
      Environment=inContainer=true
      Environment=nodeName=tdarr.calculon.tigc.eu
      PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:8265:8265/tcp
      PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:8265:8265/tcp
      Volume=#{tdarr_root}/server:/app/server
      Volume=#{tdarr_root}/configs:/app/configs
      Volume=#{tdarr_root}/logs:/app/logs
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/movies:/media/movies
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/series:/media/series
      Volume=#{tdarr_root}/cache/movies:/var/cache/transcode/movies
      Volume=#{tdarr_root}/cache/series:/var/cache/transcode/series
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Tdarr Media Transcoding",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_vhost "tdarr.calculon.tigc.eu" do
  server_name "tdarr.calculon.tigc.eu"
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8265
  oauth2_proxy(
    emails: node["calculon"]["oauth2_proxy"]["secrets"]["syncthing_authenticated_emails"],
    port: 4001
  )
  cloudflare true
  action :delete
end

calculon_www_upstream "/tdarr" do
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port 8265
end
