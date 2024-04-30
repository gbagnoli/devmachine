tdarr_root = node["calculon"]["storage"]["paths"]["tdarr"]
jackett_root = node["calculon"]["storage"]["paths"]["jackett"]
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
gid = node["calculon"]["data"]["gid"]

[jackett_root, tdarr_root].each do |r|
  calculon_btrfs_volume r do
    owner user
    group group
    mode "2775"
    setfacl true
  end
end

%w[server configs logs cache cache/series cache/movies].each do |dir|
  directory "#{tdarr_root}/#{dir}" do
    owner user
    group group
    mode "2755"
  end
end

podman_image "tdarr" do
  config(
    Image: ["Image=ghcr.io/haveagitgat/tdarr"],
  )
end

podman_container "tdarr" do
  config(
    Container: %W{
      Image=tdarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=GUID=#{gid}
      Environment=serverPort=8266
      Environment=webUIPort=8265
      Environment=internalNode=true
      Environment=inContainer=true
      Environment=nodeName=tdarr.calculon.tigc.eu
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

directory node["calculon"]["storage"]["paths"]["blackhole"] do
  group group
  owner user
  mode "2775"
end

podman_image "jackett" do
  config(
    Image: ["Image=lscr.io/linuxserver/jackett:latest"],
  )
end

podman_container "jackett" do
  config(
    Container: %W{
      Image=jackett.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=GUID=#{gid}
      Volume=#{jackett_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["blackhole"]}:/downloads
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Jackett Indexer",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_www_upstream "/tdarr" do
  upstream_port 8265
  title "Tdarr (Transcoding)"
end

calculon_www_upstream "/jackett" do
  upstream_port 9117
  title "Jackett (Indexer)"
end
