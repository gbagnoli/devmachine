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
    Image: ["Image=pihole/pihole:latest"],
  )
end

podman_container "pihole" do
  config(
    Container: %w{
      Network=bridge
      Image=pihole.image
      Volume=/etc/pihole/conf:/etc/pihole
      Volume=/etc/pihole/dnsmasq.d/etc/dnsmasq.d
      Dns=1.1.1.1
      Dns=127.0.0.1
      Hostname=pihole.rupik.tigc.eu
      Environment=VIRTUAL_HOST=pi.hole
      Environment=PROXY_LOCATION=pi.hole
      Environment=TZ=Europe/Madrid
      Environment=ServerIP=127.0.0.1
      PublishPort=80:80/tcp
      PublishPort=443:443/tcp
      PublishPort=53:53/tcp
      PublishPort=53:53/udp
    },
    Service: [
      "Restart=always",
    ],
    Unit: [
      "Description=Pi. Hole",
      "Wants=network.target",
      "After=network-online.target",
      "Before=tailscale.service",
    ],
    Install: [
      "WantedBy=multi-user.target default.target",
    ]
  )
end

file "/usr/local/bin/update_pihole" do
  action :delete
end

cron "update pihole image" do
  command "/usr/local/bin/update_pihole &> /var/log/update_pihole.log"
  minute "18"
  hour "4"
  user "root"
  action :delete
end
