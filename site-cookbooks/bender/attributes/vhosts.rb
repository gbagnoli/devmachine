default["bender"]["certificates"]["directory"] = "/etc/ssl/containers"

default["bender"]["vhosts"]["bender.tigc.eu"] = {
  # for nginx. either a string, or an array of strings
  server_name: "bender.tigc.eu",
  # either you set the container, or set upstream_url
  container: "marvin",
  # either http or https
  upstream_protocol: "http",
  # for letsencrypt you also need ssl: true
  ssl: true,
  # this NEEDS the dns pointing at the box! or chef
  # will fail. However, verification is done on bender,
  # so upstream does not have to be alive.
  letsencrypt: true,
  # if empty it will use the first server_name
  #  letsencrypt_common_name: 'bender.tigc.eu',
  letsencrypt_alt_names: [],
  # restricts real-ip headers from cloudflare ips
  cloudflare: true,

# other options:
# port (best left alone)
# letsencrypt_contact (The contact to use for the certificate)
}

default["bender"]["vhosts"]["sync.tigc.eu"] = {
  server_name: "sync.tigc.eu",
  container: "marvin",
  upstream_protocol: "http",
  ssl: true,
  letsencrypt: true,
  cloudflare: true,
}

default["bender"]["vhosts"]["chat.tigc.eu"] = {
  server_name: "chat.tigc.eu",
  container: "marvin",
  upstream_protocol: "http",
  ssl: true,
  letsencrypt: true,
  cloudflare: true,
}

# rubocop:disable Layout/LineLength
default["bender"]["vhosts"]["media.tigc.eu"] = {
  server_name: "media.tigc.eu",
  container: "flexo",
  upstream_protocol: "http",
  ssl: true,
  letsencrypt: true,
  cloudflare: true,
  proxy_caches: {
    "/var/cache/nginx/jellyfin-videos" => "levels=1:2 keys_zone=jellyfin-videos:100m inactive=90d max_size=35000m",
    "/var/cache/nginx/jellyfin" => "levels=1:2 keys_zone=jellyfin:100m max_size=15g inactive=30d use_temp_path=off",
  },
  maps: [
    "$request_uri $h264Level { ~(h264-level=)(.+?)& $2; }",
    "$request_uri $h264Profile { ~(h264-profile=)(.+?)& $2; }",
  ],
  extra_config: <<EOH
  set $jellyfin 172.24.24.3;
  location #{node["flexo"]["jellyfin"]["base_url"]} {
      return 302 $scheme://$host/player/;
  }

  location ~* ^/jellyfin/Videos/(.*)/(?!live) {
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

  location ~ /jellyfin/Items/(.*)/Images {
      access_log /var/log/nginx/jellyfin-cache.log;
      proxy_pass http://$jellyfin:8096;
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

  location #{node["flexo"]["jellyfin"]["base_url"]}/ {
      proxy_pass http://$jellyfin:8096;
      access_log /var/log/nginx/jellyfin.log;
      proxy_pass_request_headers on;
      proxy_set_header Host $host;
      proxy_set_header X-Real-IP $remote_addr;
      proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header X-Forwarded-Proto $scheme;
      proxy_set_header X-Forwarded-Host $http_host;
      proxy_set_header Upgrade $http_upgrade;
      proxy_set_header Connection $http_connection;
      # Disable buffering when the nginx proxy gets very resource heavy upon streaming
      proxy_buffering off;
  }
EOH
}
# rubocop:enable Layout/LineLength
