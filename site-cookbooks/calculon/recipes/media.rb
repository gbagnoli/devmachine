chef_gem 'toml-rb' do
  compile_time true
end

tdarr_root = node["calculon"]["storage"]["paths"]["tdarr"]
prowlarr_root = node["calculon"]["storage"]["paths"]["prowlarr"]
putioarr_root = node["calculon"]["storage"]["paths"]["putioarr"]
radarr_root = node["calculon"]["storage"]["paths"]["radarr"]
sonarr_root = node["calculon"]["storage"]["paths"]["sonarr"]
lidarr_root = node["calculon"]["storage"]["paths"]["lidarr"]
jellyfin_root = node["calculon"]["storage"]["paths"]["jellyfin"]
plex_root = node["calculon"]["storage"]["paths"]["plex"]
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
ipv6 = node["calculon"]["network"]["containers"]["ipv6"]["addr"]
ipv4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]
gid = node["calculon"]["data"]["gid"]

[ jellyfin_root,
  putioarr_root,
  radarr_root,
  sonarr_root,
  tdarr_root,
  prowlarr_root,
  lidarr_root,
  plex_root,
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

%w[config transcode].each do |dir|
  directory "#{plex_root}/#{dir}" do
    owner user
    group group
    mode "2755"
  end
end

podman_image "tdarr" do
  config(
    Image: ["Image=ghcr.io/haveagitgat/tdarr"],
  )
  action :delete
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
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/movies/library:/media/movies
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/series/library:/media/series
      Volume=#{tdarr_root}/cache/movies:/var/cache/transcode/movies
      Volume=#{tdarr_root}/cache/series:/var/cache/transcode/series
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Tdarr Media Transcoding",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
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
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
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
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/movies:/movies
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Radarr",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
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
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/series:/tv
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=sonarr",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

podman_image "lidarr" do
  config(
    Image: ["Image=lscr.io/linuxserver/lidarr:latest"],
  )
end

podman_container "lidarr" do
  config(
    Container: %W{
      Image=lidarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{lidarr_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}/music:/music
      Volume=#{node["calculon"]["storage"]["paths"]["downloads"]}:/downloads
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=lidarr",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

podman_image "putioarr" do
  config(
    Image: ["Image=ghcr.io/wouterdebie/putioarr:latest"],
  )
end

directory putioarr_root do
  owner user
  group group
  mode "2755"
end

config_hash = {
  username: "servarr",
  password: node["putioarr"]["passwd"],
  download_directory: "/downloads",
  port: 9091,
  loglevel: "info",
  uid: uid.to_i,
  polling_interval: 10,
  skip_directories: %w(sample extra),
  orchestration_workes:10,
  download_workers: 4,
  putio: {
    api_key: node["putioarr"]["putio_api_key"],
  },
}

{
  radarr: {
    "address" => "[::1]",
    "port" => 7878,
    "api_key" => node["putioarr"]["radarr_api_key"]
  },
  sonarr: {
    "address" => "[::1]",
    "port" => 8989,
    "api_key" => node["putioarr"]["sonarr_api_key"]
  },
  lidarr: {
    "address" => "[::1]",
    "port" => 8686,
    "api_key" => node["putioarr"]["lidarr_api_key"],
  },
}.each do |service, conf|
  config_hash[service] = {
    url: "http://[::1]:#{conf["port"]}/#{service}",
    api_key: conf["api_key"],
  }
end

file "#{putioarr_root}/config.toml" do
  owner user
  group group
  mode "0700"
  content lazy { to_toml(config_hash) }
  notifies :restart, "service[putioarr]", :delayed
end

podman_container "putioarr" do
  config(
    Container: %W{
      Image=putioarr.image
      Pod=web.pod
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PUID=#{uid}
      Environment=PGID=#{gid}
      Volume=#{putioarr_root}:/config
      Volume=#{node["calculon"]["storage"]["paths"]["downloads"]}:/downloads
      ExposeHostPort=9091
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Putio proxy",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

service "putioarr" do
  action %i{start}
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
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/series/library:/dara/tvshows
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/movies/library:/data/movies
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/yoga-pilates/library:/data/yoga-pilates
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}/music:/data/music
      PublishPort=[#{ipv6}]:8096:8096/tcp
      PublishPort=#{ipv4}:8096:8096/tcp
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=jellyfin media server",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

podman_image "plex" do
  config(
    Image: ["Image=docker.io/plexinc/pms-docker:latest"],
  )
end

publishports = []
openports = []
{tcp: %w{32400 8324 32469 1900}, udp: %w{32410 32412 32413 32414}}.each do |proto, ports|
  ports.each do |port|
    publishports << "PublishPort=[::]:#{port}:#{port}/#{proto}"
    publishports << "PublishPort=#{port}:#{port}/#{proto}"
    openports << "#{port}/#{proto}"
  end
end

calculon_firewalld_port "syncthing" do
  port openports
end

podman_container "plex" do
  config(
    Container: %W{
      Image=plex.image
      Network=calculon.network
      HostName=plex.tigc.eu
      Environment=TZ=#{node["calculon"]["TZ"]}
      Environment=PLEX_UID=#{uid}
      Environment=PLEX_GID=#{gid}
      Environment=CHANGE_CONFIG_DIR_OWNERSHIP=false
      Volume=#{plex_root}/config:/config
      Volume=#{plex_root}/transcode:/transcode
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/series/library:/data/tvshows
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/movies/library:/data/movies
      Volume=#{node["calculon"]["storage"]["paths"]["media"]}/yoga-pilates/library:/data/yoga-pilates
      Volume=#{node["calculon"]["storage"]["paths"]["sync"]}/music:/data/music
    } + publishports,
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=jellyfin media server",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

calculon_www_upstream "/tdarr" do
  upstream_port 8265
  title "Transcoding"
  upgrade true
  category "Tools"
  upgrade "$http_connection"
end

calculon_www_upstream "/radarr" do
  upstream_port 7878
  title "Movies"
  matcher "^~"
  category "Media"
  upgrade "$http_connection"
end

calculon_www_upstream "/sonarr" do
  upstream_port 8989
  title "Series"
  matcher "^~"
  category "Media"
  upgrade "$http_connection"
end

calculon_www_upstream "/lidarr" do
  upstream_port 8686
  title "Music"
  matcher "^~"
  category "Media"
  upgrade "$http_connection"
end

calculon_www_upstream "/prowlarr" do
  upstream_port 9696
  title "Indexer"
  matcher "^~"
  category "Tools"
  upgrade "$http_connection"
end

domain = node["calculon"]["www"]["media_domain"]
return if domain.nil?

calculon_www_link "Jellyfin" do
  category "Media"
  url "https://#{domain}/"
end

jfmap = <<~EOH
    $sent_http_content_type $jf_content {
		# Undefined content is off
		"default" "";
		# HTML gets unique define
		"text/html" ", epoch";
		# Text content, 30 days / 1 month
		"text/javascript" ", max-age=2592000";
		"text/css" ", max-age=2592000";
		# Fonts, 365 days / 12 months
		"application/vnd.ms-fontobject" ", max-age=31536000";
		"application/font-woff" ", max-age=31536000";
		"application/x-font-truetype" ", max-age=31536000";
		"application/x-font-opentype" ", max-age=31536000";
		"~font/" "max-age=31536000";
		# Media, 180 days / 6 months
		# You don't want to send cache responses for video or audio.
		"~image/" ", max-age=15552000";
	}
EOH

podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    disable_default_location true
    proxy_caches(
      "jellyfin-videos" => "levels=1:2 keys_zone=jellyfin-videos:100m inactive=90d max_size=35000m",
      "jellyfin" => "levels=1:2 keys_zone=jellyfin:100m max_size=15g inactive=30d use_temp_path=off",
    )
    maps [
      "$request_uri $h264Level { ~(h264-level=)(.+?)& $2; }",
      "$request_uri $h264Profile { ~(h264-profile=)(.+?)& $2; }",
      jfmap,
    ]
    extra_config <<~EOH
    add_header Cache-Control "private$jf_content";
    add_header Content-Security-Policy "base-uri 'none'; connect-src 'self'; default-src 'none'; font-src 'self' data:; form-action 'self'; frame-ancestors 'self'; frame-src 'self'; img-src 'self' blob: data: https:; manifest-src 'self'; media-src 'self' blob:; object-src 'none'; script-src 'self' 'unsafe-inline' blob:; script-src-elem 'self' https://www.gstatic.com/cv/js/sender/v1/cast_sender.js https://www.gstatic.com/eureka/clank/ blob:; style-src 'self' 'unsafe-inline'; worker-src 'self' blob:;";

    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
    add_header X-Frame-Options SAMEORIGIN always;
    add_header X-Content-Type-Options nosniff;
    add_header Referrer-Policy strict-origin-when-cross-origin;
    add_header Alt-Svc 'h3-32=":$server_port"; ma=86400, h3=":$server_port"; ma=86400';
    add_header X-protocol $server_protocol always;
    set $jellyfin #{ipv4};
    if ($request_method !~ ^(GET|HEAD|POST|DELETE)$ ) {
    	return 405;
    }

    sendfile on;
    tcp_nopush on;

    location = / {
    	return 301 https://$host/web/;
    }

    location / {
    	proxy_pass http://$jellyfin:8096;
    	proxy_http_version 1.1;
    	proxy_set_header Host $host;
    	proxy_set_header X-Real-IP $remote_addr;
    	proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    	proxy_set_header X-Forwarded-Proto $scheme;
    	proxy_set_header X-Forwarded-Protocol $scheme;
    	proxy_set_header X-Forwarded-Host $http_host;
    	proxy_buffering on;
    	proxy_buffers 16 4k;
    	proxy_buffer_size 4k;
    	proxy_busy_buffers_size 8k;
    	proxy_temp_file_write_size 8k;
    	proxy_max_temp_file_size 16k;
    	proxy_connect_timeout 60s;
    	proxy_send_timeout 60s;
    	proxy_read_timeout 60s;
    }

    location = /web/ {
    	proxy_pass http://$jellyfin:8096/web/index.html;
    	proxy_http_version 1.1;
    	proxy_set_header Host $host;
    	proxy_set_header X-Real-IP $remote_addr;
    	proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    	proxy_set_header X-Forwarded-Proto $scheme;
    	proxy_set_header X-Forwarded-Protocol $scheme;
    	proxy_set_header X-Forwarded-Host $http_host;
    }

    location /socket {
    	proxy_pass http://$jellyfin:8096;
    	proxy_http_version 1.1;
    	proxy_set_header Upgrade $http_upgrade;
    	proxy_set_header Connection "upgrade";
    	proxy_set_header Host $host;
    	proxy_set_header X-Real-IP $remote_addr;
    	proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    	proxy_set_header X-Forwarded-Proto $scheme;
    	proxy_set_header X-Forwarded-Protocol $scheme;
    	proxy_set_header X-Forwarded-Host $http_host;
    	proxy_connect_timeout 60s;
    	proxy_send_timeout 60s;
    	proxy_read_timeout 60s;
    }

    location ~ /Items/(.*)/Images {
    	proxy_pass http://$jellyfin:8096;
    	proxy_http_version 1.1;
    	proxy_set_header Host $host;
    	proxy_set_header X-Real-IP $remote_addr;
    	proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    	proxy_set_header X-Forwarded-Proto $scheme;
    	proxy_set_header X-Forwarded-Protocol $scheme;
    	proxy_set_header X-Forwarded-Host $http_host;
    	proxy_cache jellyfin;
    	proxy_cache_revalidate on;
    	proxy_cache_lock on;
    	# add_header X-Cache-Status $upstream_cache_status; # This is only to check if cache is working
    }
    location ~* ^/Videos/(.*)/(?!live) {
      # Set size of a slice (this amount will be always requested from the backend by nginx)
      # Higher value means more latency, lower more overhead
      # This size is independent of the size clients/browsers can request
      slice 2m;

      access_log /var/log/nginx/jellyfin-cache.log;
      proxy_cache jellyfin-videos;
      proxy_cache_valid 200 206 301 302 30d;
      proxy_ignore_headers Expires Cache-Control Set-Cookie X-Accel-Expires;
      proxy_cache_use_stale error timeout invalid_header updating http_500 http_502 http_503 http_504;
      proxy_connect_timeout 15s;
      proxy_http_version 1.1;
      proxy_set_header Connection "";
      # Transmit slice range to the backend
      proxy_set_header Range $slice_range;

      # This saves bandwidth between the proxy and jellyfin, as a file is only
      # downloaded one time instead of multiple times when multiple clients
      # want to at the same time
      # The first client will trigger the download, the other clients will have
      # to wait until the slice is cached
      # Esp. practical during SyncPlay
      proxy_cache_lock on;
      proxy_cache_lock_age 60s;

      proxy_pass http://$jellyfin:8096;
      proxy_cache_key "jellyvideo$uri?MediaSourceId=$arg_MediaSourceId&VideoCodec=$arg_VideoCodec&AudioCodec=$arg_AudioCodec&AudioStreamIndex=$arg_AudioStreamIndex&VideoBitrate=$arg_VideoBitrate&AudioBitrate=$arg_AudioBitrate&SubtitleMethod=$arg_SubtitleMethod&TranscodingMaxAudioChannels=$arg_TranscodingMaxAudioChannels&RequireAvc=$arg_RequireAvc&SegmentContainer=$arg_SegmentContainer&MinSegments=$arg_MinSegments&BreakOnNonKeyFrames=$arg_BreakOnNonKeyFrames&h264-profile=$h264Profile&h264-level=$h264Level&slicerange=$slice_range";

      # add_header X-Cache-Status $upstream_cache_status; # This is only for debugging cache
    }

    location ~\.(pl|cgi|py|sh|lua|asp|php)$ {
    	return 444;
    }
    EOH
end

systemd_unit "servarr_delete_stale_downloads.service" do
  content <<~EOH
   [Unit]
   Description=Remove stale downloads older than a week

   [Service]
   Type=oneshot
   ExecStart=/bin/find #{node["calculon"]["storage"]["paths"]["downloads"]} -type f -mtime +7 -delete
   ExecStart=/bin/find #{node["calculon"]["storage"]["paths"]["downloads"]} -type d -mtime +7 -delete
   User=root
   Group=systemd-journal
	EOH
  action %i(create enable)
end

systemd_unit "servarr_delete_stale_downloads.timer" do
  content <<~EOH
    [Unit]
    Description=Periodically cleanup old downloads

    [Timer]
    Unit=servarr_delete_stale_downloads.service
    Persistent=true
    OnCalendar=daily

    [Install]
    WantedBy=timers.target
  EOH
  action %i(create enable start)
end
