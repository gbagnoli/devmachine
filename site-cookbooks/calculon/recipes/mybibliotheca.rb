web_ext_port = 59010
web_port = 5054

user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
domain = node["calculon"]["mybibliotheca"]["domain"]
backup_path = "#{node["calculon"]["storage"]["paths"]["backups"]}/mybibliotheca"
mybibliotheca_root = node["calculon"]["storage"]["paths"]["mybibliotheca"]
data = "#{mybibliotheca_root}/data"
envfile="/etc/containers/systemd/mybibliotheca.env"

container = "mybibliotheca"
service = "#{container}.service"

service service do
  action :stop
end


file envfile do
  mode "0400"
  action :delete
end

podman_container container do
  config(
    Container: [
      "Image=docker.io/pickles4evaaaa/mybibliotheca:2.1.0",
      "EnvironmentFile=#{envfile}",
      "Volume=#{data}:/app/data",
      "PublishPort=[::]:#{web_ext_port}:#{web_port}/tcp",
      "PublishPort=#{web_ext_port}:#{web_port}/tcp",
    ],
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=MyBibliotheca",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
  action :delete
end

calculon_btrfs_volume mybibliotheca_root do
  owner user
  group group
  mode "2775"
  setfacl true
  action :delete
end

[
  "data",
].each do |dir|
  directory "#{mybibliotheca_root}/#{dir}" do
      owner user
      group group
      mode "2755"
      action :delete
  end
end

addr4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]
podman_nginx_vhost domain do
  server_name domain
  cloudflare true
  upgrade true
  upstream_port web_ext_port
  # for some reason ipv6 is not responding
  upstream_address addr4
  oauth2_proxy(
    emails: node["calculon"]["www"]["user_emails"],
    port: 4300
  )
  action :delete
end

systemd_unit "mybibliotheca-backup-files.service" do
  content <<~EOH
[Unit]
Description=Daily Mybibliotheca Files Backup

[Service]
Type=oneshot
ExecStart=/usr/bin/rsync -aAXHv --mkpath --delete #{mybibliotheca_root}/ #{backup_path}/

[Install]
WantedBy=default.target
  EOH
  action %i(disable delete)
end

systemd_unit "wanderer-backup-files.timer" do
  content <<~EOH
[Unit]
Description=Run Mybibliotheca Files Backup Daily

[Timer]
OnCalendar=daily
RandomizedDelaySec=4h
Persistent=true

[Install]
WantedBy=timers.target
  EOH
  action %i(stop disable delete)
end
