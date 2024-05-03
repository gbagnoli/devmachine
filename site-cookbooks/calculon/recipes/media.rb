require 'toml'

tdarr_root = node["calculon"]["storage"]["paths"]["tdarr"]
prowlarr_root = node["calculon"]["storage"]["paths"]["prowlarr"]
putioarr_root = node["calculon"]["storage"]["paths"]["putioarr"]
radarr_root = node["calculon"]["storage"]["paths"]["radarr"]
sonarr_root = node["calculon"]["storage"]["paths"]["sonarr"]
jellyfin_root = node["calculon"]["storage"]["paths"]["jellyfin"]
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
gid = node["calculon"]["data"]["gid"]

[ jellyfin_root,
  putioarr_root,
  radarr_root,
  sonarr_root,
  tdarr_root,
  prowlarr_root,
].each do |r|
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
      Environment=PGID=#{gid}
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
  action :delete
end

podman_image "prowlarr" do
  config(
    Image: ["Image=lscr.io/linuxserver/prowlarr:latest"],
  )
end

podman_container "prowlarr" do
  config(
    Container: %W{
      Image=prowlarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{prowlarr_root}:/config
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Prowlarr Indexer",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

podman_image "radarr" do
  config(
    Image: ["Image=lscr.io/linuxserver/radarr:latest"],
  )
end

podman_container "radarr" do
  config(
    Container: %W{
      Image=radarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{radarr_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/movies:/movies
      Volume=#{node["calculon"]["storage"]["paths"]["downloads"]}/movies:/downloads
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Radarr",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

podman_image "sonarr" do
  config(
    Image: ["Image=lscr.io/linuxserver/sonarr:latest"],
  )
end

podman_container "sonarr" do
  config(
    Container: %W{
      Image=sonarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{sonarr_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/series:/tv
      Volume=#{node["calculon"]["storage"]["paths"]["downloads"]}/series:/downloads
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=sonarr",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

podman_image "putioarr" do
  config(
    Image: ["Image=ghcr.io/wouterdebie/putioarr:latest"],
  )
end

services = {
  "radarr" => {
    "address" => "[::1]",
    "port" => 7878,
    "api_key" => node["putioarr"]["radarr_api_key"]
  },
  "sonarr" => {
    "address" => "[::1]",
    "port" => 8989,
    "api_key" => node["putioarr"]["sonarr_api_key"]
  }
}

node["calculon"]["storage"]["library_dirs"].sort.each do |library, libconf|
  configd = "#{putioarr_root}/#{library}"
  download_dir = "#{node["calculon"]["storage"]["paths"]["downloads"]}/#{library}"
  library_dir = "#{node["calculon"]["storage"]["paths"]["library"]}/#{library}"
  port = libconf["putioarr_port"]
  service = services[libconf["service"]]
  next if service.nil?

  directory configd do
    owner user
    group group
    mode "2755"
  end

  config_hash = {
    username: libconf["service"],
    password: node["putioarr"]["#{libconf["service"]}_passwd"],
    download_directory: "/downloads",
    port: port,
    loglevel: "info",
    uid: uid.to_i,
    polling_interval: 10,
    skip_directories: %w(sample extra),
    orchestration_workes:10,
    download_workers: 4,
    putio: {
      api_key: node["putioarr"]["putio_api_key"],
    },
    libconf["service"].to_sym => {
      url: "http://[::1]:#{service["port"]}/#{libconf["service"]}",
      api_key: service["api_key"],
    },
  }

  file "#{configd}/config.toml" do
    owner user
    group group
    mode "0700"
    content TOML::Generator.new(config_hash).body
    notifies :restart, "service[putioarr-#{library}]", :delayed
  end

  podman_container "putioarr-#{library}" do
    config(
      Container: %W{
        Image=putioarr.image
        Pod=web.pod
        Environment=TZ=#{node["calculon"]["TZ"]}
        Environment=PUID=#{uid}
        Environment=PGID=#{gid}
        Volume=#{configd}:/config
        Volume=#{node["calculon"]["storage"]["paths"]["downloads"]}/#{library}:/downloads
        ExposeHostPort=#{port}
      },
      Service: %w{
        Restart=always
      },
      Unit: [
        "Description=Putio proxy for #{library}",
        "After=network-online.target",
        "Wants=network-online.target",
      ],
      Install: %w{
        WantedBy=multi-user.target
      }
    )
  end
  service "putioarr-#{library}" do
    action %i{start enable}
  end
end

podman_image "jellyfin" do
  config(
    Image: ["Image=lscr.io/linuxserver/jellyfin:latest"],
  )
end

podman_container "jellyfin" do
  config(
    Container: %W{
      Image=jellyfin.image
      Network=calculon.network
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{jellyfin_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/series:/dara/tvshows
      Volume=#{node["calculon"]["storage"]["paths"]["library"]}/movies:/data/movies
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}/music:/data/music
      PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:8096:8096/tcp
      PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:8096:8096/tcp
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=jellyfin media server",
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
  title "Transcoding"
  upgrade true
  category "Tools"
end

calculon_www_upstream "/radarr" do
  upstream_port 7878
  title "Movies"
  matcher "^~"
  category "Media"
end

calculon_www_upstream "/sonarr" do
  upstream_port 8989
  title "Series"
  matcher "^~"
  category "Media"
end

calculon_www_upstream "/prowlarr" do
  upstream_port 9696
  title "Indexer"
  matcher "^~"
  category "Tools"
end

calculon_www_upstream "/jellyfin" do
  upstream_address "#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}"
  upstream_port 8096
  title "Jellyfin"
  upgrade true
  extra_properties(
    client_max_body_size: "20M",
    add_header: 'Cross-Origin-Opener-Policy "same-origin" always',
    add_header: 'Cross-Origin-Embedder-Policy "require-corp" always',
    add_header: 'Cross-Origin-Resource-Policy "same-origin" always',
    proxy_buffering: "off",
  )
  matcher ""  # empty matcher to avoid the /
  category "Media"
end
