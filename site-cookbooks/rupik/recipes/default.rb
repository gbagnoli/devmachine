include_recipe "rupik::mounts"
include_recipe "argonone"
include_recipe "rupik::podman"
include_recipe "rupik::nginx"
include_recipe "rupik::pihole"
include_recipe "rupik::unifi"
include_recipe "rupik::btrbk"
include_recipe "syncthing"
include_recipe "rupik::tailscale"

domain = node["rupik"]["www"]["pihole_domain"]
unless domain.nil?
  podman_nginx_vhost domain do
    server_name domain
    disable_default_location true
    extra_config <<~EOH
      location /admin/ {
        # lack of trailing / is significant as we want to
        # pass /admin to the url.
        proxy_pass http://localhost:8888;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Server $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $http_connection;
      }
    EOH
  end
end
