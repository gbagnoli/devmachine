jp_root = node["calculon"]["storage"]["paths"]["joplin"]
db_path = "#{jp_root}/database"
backup_path = "#{node["calculon"]["storage"]["paths"]["backups"]}/joplin"
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
joplin_port=22_300
pgport = 5959
pgdb = "joplin"
pguser = "joplin"
pgpasswd = node["calculon"]["joplin"]["secrets"]["pgpasswd"]
if pgpasswd.nil?
  Chef::Log.info('calculon: missing secrets for postgresql')
  raise
end
domain = node["calculon"]["www"]["notes_domain"]
joplin_base_url = "https://#{domain}/"
joplin_base_url = "https://#{node["calculon"]["www"]["domain"]}/joplin" if domain.nil?

calculon_btrfs_volume jp_root do
  owner user
  group group
  mode "2775"
  setfacl true
end

[db_path, backup_path].each do |dir|
  directory dir do
    owner user
    group group
    mode "2755"
  end
end

podman_image "joplin" do
  config(
    Image: ["Image=docker.io/joplin/server:latest"],
  )
end

podman_image "postgres" do
  config(
    Image: ["Image=docker.io/postgres:18"],
  )
end

podman_container "joplin-db" do
  config(
    Container: [
      "Image=postgres.image",
      "Pod=web.pod",
      "Exec=-p #{pgport}",
      "Environment=POSTGRES_DATABASE=#{pgdb}",
      "Environment=POSTGRES_USER=#{pguser}",
      "Environment=POSTGRES_PASSWORD=#{pgpasswd}",
      "Volume=#{db_path}:/var/lib/postgres/data",
      "HealthCmd=pg_isready -p #{pgport} -U #{pguser} -d #{pgdb}",
      "HealthInterval=5s",
      "HealthRetries=5",
    ],
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Joplin Postgresql Database",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

mail_config = []
mconf = node["calculon"]["joplin"]["secrets"]["mail"]
unless mconf.nil?
  mail_config = %W{
    Environment=MAILER_ENABLED=1
    Environment=MAILER_HOST=#{mconf["host"]}
    Environment=MAILER_PORT=#{mconf["port"]}
    Environment=MAILER_SECURITY=#{mconf["security"]}
    Environment=MAILER_AUTH_USER=#{mconf["username"]}
    Environment=MAILER_AUTH_PASSWORD=#{mconf["password"]}
    Environment=MAILER_NOREPLY_NAME=#{mconf["mailer_name"]}
    Environment=MAILER_NOREPLY_EMAIL=#{mconf["mailer_email"]}
    Environment=SUPPORT_EMAIL=#{mconf['support_email']}
    Environment=SUPPORT_NAME=#{mconf['support_name']}
  }
end

podman_container "joplin-server" do
  config(
    Container: %W{
      Image=joplin.image
      Pod=web.pod
      Environment=DB_CLIENT=pg
      Environment=POSTGRES_DATABASE=#{pgdb}
      Environment=POSTGRES_USER=#{pguser}
      Environment=POSTGRES_PASSWORD=#{pgpasswd}
      Environment=POSTGRES_HOST=::1
      Environment=POSTGRES_PORT=#{pgport}
      Environment=APP_PORT=#{joplin_port}
      Environment=APP_BASE_URL=#{joplin_base_url}
      Annotation=run.oci.condition-wait=joplin-db.service:healthy
      Environment=MAX_TIME_DRIFT=0
      Volume=/etc/localtime:/etc/localtime:ro
    } + mail_config,
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Joplin Server",
      "After=joplin-db.service",
      "Requires=joplin-db.service",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

template "/usr/local/bin/joplin_backup" do
  source "joplin_backup.erb"
  variables(
    backup_dir: backup_path,
    db_port: pgport,
    db_user: pguser,
    db: pgdb,
  )
  mode '0755'
end

template "/usr/local/bin/joplin_restore" do
  source "joplin_restore.erb"
  variables(
    backup_dir: backup_path,
    db_port: pgport,
    db_user: pguser,
    db: pgdb,
  )
  mode '0755'
end

systemd_unit 'joplin-backup.service' do
  content <<~EOH
[Unit]
Description=Daily Joplin Postgres Backup
After=joplin-db.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/joplin_backup

[Install]
WantedBy=default.target
  EOH
  action %i(create enable)
end

systemd_unit 'joplin-backup.timer' do
  content <<~EOH
[Unit]
Description=Run Joplin Backup Daily

[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true

[Install]
WantedBy=timers.target
  EOH
  action %i(create enable start)
end

if domain.nil?
  # if we don't have a domain, at least setup the upstream
  calculon_www_upstream "/joplin" do
    upstream_port joplin_port
    title "Notes"
    category "Apps"
    upgrade "$http_connection"
    extra_properties [
      "client_max_body_size 100m",
      "proxy_read_timeout 86400s",
      "proxy_send_timeout 86400s",
    ]
  end
  # return - the rest is for the external domain
  return
end

calculon_www_link "Notes" do
  category "Apps"
  url joplin_base_url
end

podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    upstream_port joplin_port
    default_location_extra_config <<~EOH
      client_max_body_size 100m;
      proxy_read_timeout 86400s;
      proxy_send_timeout 86400s;
    EOH

end
