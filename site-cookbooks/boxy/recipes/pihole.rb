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

directory "/etc/pihole"
directory "/etc/pihole/conf"
directory "/etc/pihole/dnsmasq.d"

podman_image "pihole" do
  config(
    Image: ["Image=docker.io/pihole/pihole:latest"],
  )
end

podman_container "pihole" do
  config(
    Container: %w{
      Pod=web.pod
      Image=pihole.image
      Volume=/etc/pihole/conf:/etc/pihole
      Volume=/etc/pihole/dnsmasq.d/etc/dnsmasq.d
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
