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

default["bender"]["vhosts"]["media.tigc.eu"] = {
  server_name: "media.tigc.eu",
  container: "flexo",
  upstream_protocol: "http",
  ssl: true,
  letsencrypt: true,
  cloudflare: true,
  extra_config: <<EOH
  # Jellyfin does not use oauth_proxy
  location #{node["flexo"]["jellyfin"]["base_url"]} {
      return 302 $scheme://$host/player/;
  }

  location #{node["flexo"]["jellyfin"]["base_url"]}/ {
      proxy_pass http://172.24.24.3:8096;
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
