web_ext_port = 59010
web_port = 5054

user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
gid = node["calculon"]["data"]["gid"]
secrets = node["calculon"]["mybibliotheca"]["secrets"]
domain = node["calculon"]["mybibliotheca"]["domain"]
backup_path = "#{node["calculon"]["storage"]["paths"]["backups"]}/mybibliotheca"
mybibliotheca_root = node["calculon"]["storage"]["paths"]["mybibliotheca"]
data = "#{mybibliotheca_root}/data"

if secrets.empty?
  Chef::Log.info('calculon: missing secrets for wanderer')
  raise
end
if domain.nil? || domain.empty?
  Chef::Log.info('calculon: missing domain for wanderer')
  raise
end
public_url = "https://#{domain}"
envfile="/etc/containers/systemd/mybibliotheca.env"

%w{secret_key security_password_salt}.each do |s|
  if secrets[s].nil? || secrets[s].empty?
    Chef::Log.info("calculon: missing secret calculon.wanderer.#{s} for wanderer")
    raise
  end
end

calculon_btrfs_volume mybibliotheca_root do
  owner user
  group group
  mode "2775"
  setfacl true
end

directory data do
    owner user
    group group
    mode "2755"
  end

config = {
  "SECRET_KEY" => secrets["secret_key"],
  "SECURITY_PASSWORD_SALT" => secrets["security_password_salt"],
  "TIMEZONE" => "Europe/Madrid",
  "WORKERS" => "12"
}

file envfile do
  mode "0400"
  content config.sort.map {|k,v| "#{k.upcase}=#{v}"}.join("\n")
end

container = "mybibliotheca"
service = "#{container}.service"

podman_container container do
  config(
    Container: [
      "Image=docker.io/pickles4evaaaa/mybibliotheca:2.1.0",
      "EnvironmentFile=#{envfile}",
      "Volume=#{data}:/app/data",
      "User=#{uid}",
      "Group=#{gid}",
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
end

service service do
  action :start
  subscribes :restart, "file[#{envfile}]"
end

calculon_www_link "Books" do
  category "Apps"
  url public_url
end

addr6 = node["calculon"]["network"]["containers"]["ipv6"]["addr"]
podman_nginx_vhost domain do
  server_name domain
  cloudflare true
  upgrade true
  upstream_address addr6
  upstream_port web_ext_port
  oauth2_proxy(
    emails: node["calculon"]["www"]["user_emails"],
    port: 4300
  )
end

systemd_unit "mybibliotheca-backup-files.service" do
  content <<~EOH
[Unit]
Description=Daily Mybibliotheca Files Backup

[Service]
Type=oneshot
ExecStart=/usr/bin/rsync -aAXHv --mkpath --delete #{data} #{backup_path}/

[Install]
WantedBy=default.target
  EOH
  action %i(create enable)
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
  action %i(create enable start)
end
