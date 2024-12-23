directory "/etc/pihole"
directory "/etc/pihole/conf"
directory "/etc/pihole/dnsmasq.d"

directory "/var/log/pihole" do
  mode "0755"
end

podman_image "pihole" do
  config(
    Image: ["Image=docker.io/pihole/pihole:latest"],
  )
end

unless node["podman"]["pihole"]["dns"]["custom"].nil?
  template "/etc/pihole/conf/custom.list" do
    mode "0644"
    variables(custom: node["podman"]["pihole"]["dns"]["custom"],
              fqdn: node["podman"]["pihole"]["dns"]["custom_domain"])
    source "custom.list.erb"
    notifies :restart, "service[pihole]", :delayed
    cookbook "rupik"
  end
end

podman_container "pihole" do
  config(
    Container: %w{
      Pod=web.pod
      Image=pihole.image
      Volume=/etc/pihole/conf:/etc/pihole
      Volume=/etc/pihole/dnsmasq.d/etc/dnsmasq.d
      Volume=/var/log/pihole:/var/log/pihole
      Environment=VIRTUAL_HOST=pi.hole
      Environment=PROXY_LOCATION=pi.hole
      Environment=TZ=Europe/Madrid
      Environment=WEB_PORT=8888
    } + [
      "PodmanArgs=--dns 127.0.0.1 --dns 1.1.1.1"
    ],
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Pi. Hole",
      "After=network-online.target",
      "Before=tailscale.service",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end

datadog_integration "datadog-pihole" do
  version "3.14.1"
  third_party true
end

service "pihole" do
  action :start
end
