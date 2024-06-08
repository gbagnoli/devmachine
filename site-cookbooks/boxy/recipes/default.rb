# disable stub DNS resolver for systemd-resolve
file "/etc/systemd/resolved.conf" do
  content <<~EOU
    [Resolve]
     DNSStubListener=no
  EOU
  notifies :restart, "service[systemd-resolved]", :immediately
end

link "/etc/resolv.conf" do
  to "/run/systemd/resolve/resolv.conf"
  notifies :restart, "service[systemd-resolved]", :immediately
end

service "systemd-resolved" do
  action %i(nothing)
end

include_recipe "boxy::mounts"
include_recipe "boxy::monitoring"
include_recipe "boxy::podman"
include_recipe "boxy::nginx"
include_recipe "boxy::pihole"
include_recipe "boxy::btrbk"
include_recipe "boxy::syncthing"
include_recipe "boxy::tailscale"
include_recipe "boxy::cloudflare_ddns"

domain = node["boxy"]["www"]["pihole_domain"]
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

domain = node["boxy"]["www"]["domain"]
unless domain.nil?
  podman_nginx_vhost domain do
    server_name domain
    act_as_upstream 4201
    oauth2_proxy(
      emails: node["boxy"]["www"]["user_emails"],
      port: 4200
    )
    upstream_paths(
      "/sync/boxy" => {
        "upstream" => "http://#{node["boxy"]["lan"]["ipv4"]["addr"]}:8384",
        "upgrade" => true,
        "extra_properties" => [
          "proxy_read_timeout 600s",
          "proxy_send_timeout 600s",
        ]
      })
  end
end
