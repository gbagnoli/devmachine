include_recipe "rupik::mounts"
include_recipe "argonone"
include_recipe "rupik::monitoring"
include_recipe "rupik::podman"
include_recipe "pihole::disable_systemd_resolved"
include_recipe "rupik::nginx"
node.default["pihole"]["container"]["Pod"] = "web.pod"
include_recipe "pihole"
include_recipe "rupik::unifi"
include_recipe "rupik::btrbk"
include_recipe "rupik::syncthing"
include_recipe "rupik::tailscale"
include_recipe "rupik::cloudflare_ddns"
include_recipe "rupik::eternal_terminal"

domain = node["rupik"]["www"]["pihole_domain"]
unless domain.nil?
  podman_nginx_vhost domain do
    server_name domain
    disable_default_location true
    extra_config <<~EOH
      location / {
        # lack of trailing / is significant as we want to
        # pass /admin to the url.
        proxy_pass http://localhost:8088;
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

domain = node["rupik"]["www"]["domain"]
aliases = node["rupik"]["www"]["domain_aliases"]
unless domain.nil?
  podman_nginx_vhost domain do
    server_name Array(domain) | Array(aliases).sort
    act_as_upstream 4201
    oauth2_proxy(
      emails: node["rupik"]["www"]["user_emails"],
      port: 4200
    )
    upstream_paths(
      "/sync/rupik" => {
        "upstream" => "http://#{node["rupik"]["lan"]["ipv4"]["addr"]}:8384",
        "upgrade" => true,
        "extra_properties" => [
          "proxy_read_timeout 600s",
          "proxy_send_timeout 600s",
        ]
      },
      "/sync/boxy" => {
        "upstream" => "http://boxy.ftwo.tigc.eu:8384",
        "upgrade" => true,
        "extra_properties" => [
          "proxy_read_timeout 600s",
          "proxy_send_timeout 600s",
        ]
      },
      "/pihole/boxy" => {
        "upstream" => "http://#{node["boxy"]["www"]["pihole_domain"]}",
        "upgrade" => true,
        "extra_properties" => [
          "proxy_read_timeout 600s",
          "proxy_send_timeout 600s",
          "proxy_redirect /admin/ $scheme://$host/pihole/boxy/admin/",
        ]
      },
      "/pihole/rupik" => {
        "upstream" => "http://#{node["rupik"]["www"]["pihole_domain"]}",
        "upgrade" => true,
        "extra_properties" => [
          "proxy_read_timeout 600s",
          "proxy_send_timeout 600s",
          "proxy_redirect /admin/ $scheme://$host/pihole/rupik/admin/",
        ]
      }
    )
  end
end
