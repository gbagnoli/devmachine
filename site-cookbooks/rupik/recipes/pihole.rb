include_recipe "rupik::mounts"
include_recipe "podman::install"
include_recipe "podman::api"

root = "#{node["rupik"]["storage"]["path"]}/containers"
user = node["user"]
system_containers_path = "#{root}/system"
user_containers_path = "#{root}/#{user["login"]}"

directory root do
  mode 0o755
end

[{path: system_containers_path, owner: "root", group: "root",
runroot: "/run/containers/storage", config:"/etc/containers/storage.conf"},
 {path: user_containers_path, owner: user["login"], group: user["group"], runroot: nil,
config: "/home/#{user["login"]}/.config/containers/storage.conf"}].each do |info|
  execute "create containers subvolume #{info[:path]}" do
    not_if { ::File.directory?(info[:path]) }
    command "btrfs subvolume create #{info[:path]}"
  end

  directory info[:path] do
    owner info[:owner]
    group info[:group]
    mode 0o775
  end

  directory File.dirname(info[:config]) do
    owner info[:owner]
    group info[:group]
    mode 0o775
    recursive true
  end

  template info[:config] do
    source "storage.conf.erb"
    variables(
      runroot: info[:runroot],
      graphroot: info[:path]
    )
  end
end

# disable stub DNS resolver for systemd-resolve
file "/etc/systemd/resolved.conf" do
  content <<~EOU
    [Resolve]
    #DNS=
    #FallbackDNS=
    #Domains=
    #DNSSEC=no
    #DNSOverTLS=no
    #MulticastDNS=no
    #LLMNR=no
    #Cache=no-negative
    #ReadEtcHosts=yes
    #ResolveUnicastSingleLabel=no
     DNSStubListener=no
  EOU
  notifies :restart, "service[systemd-resolved]"
end

link "/etc/resolv.conf" do
  to "/run/systemd/resolve/resolv.conf"
  notifies :restart, "service[systemd-resolved]", :immediately
end

service "systemd-resolved" do
  action %i[nothing]
end

directory "/etc/pihole"
directory "/etc/pihole/conf"
directory "/etc/pihole/dnsmasq.d"

systemd_unit "pihole.service" do
  content <<~EOU
    [Unit]
    Description=Pi.hole container
    Documentation=https://github.com/pi-hole/docker-pi-hole/#running-pi-hole-docker
    Wants=network.target
    After=network-online.target

    [Service]
    Restart=always
    TimeoutStopSec=61
    ExecStartPre=/bin/rm -f %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.pid %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.ctr-id
    ExecStart=/usr/bin/podman run --conmon-pidfile %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.pid --cidfile %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.ctr-id --cgroups=no-conmon --replace --name pihole -d -p 80:80 -p 443:443 -p 53:53/tcp -p 53:53/udp -e TZ=Europe/Dublin -v /etc/pihole/conf:/etc/pihole -v /etc/pihole/dnsmasq.d:/etc/dnsmaq.d --dns=127.0.0.1 --dns=1.1.1.1 --restart=unless-stopped --hostname pi.hole -e VIRTUAL_HOST=pi.hole -e PROXY_LOCATION=pi.hole -e ServerIP=127.0.0.1 pihole/pihole:latest
    ExecStop=/usr/bin/podman stop --ignore --cidfile %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.ctr-id -t 1
    ExecStopPost=/usr/bin/podman rm --ignore -f --cidfile %t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.ctr-id
    PIDFile=%t/container-03561845db5d7f5a8ac0729a15d9e89cd0188c276bc827d20f2eeb1976b11156.pid
    Type=forking
  EOU
  action %i[create enable start]
end
