# database
pgport = 5858
pgdb = "airtrail"
pguser = "airtrail"
pgpasswd = node["calculon"]["airtrail"]["secrets"]["pgpasswd"]
airtrail_port = 3214
domain = node["calculon"]["airtrail"]["domain"]
airtrail_base_url = "https://#{domain}"
airtrail_files = node["calculon"]["storage"]["paths"]["airtrail"]
backup_path = "#{node["calculon"]["storage"]["paths"]["backups"]}/airtrail"
db_service_unit = "postgresql-airtrail.service"

if pgpasswd.nil?
  Chef::Log.info('calculon: missing secrets for postgresql')
  raise
end

calculon_postgresql pgdb do
  port pgport
  user pguser
  password pgpasswd
  podman_pod "web.pod"
end

podman_image "airtrail" do
  config(
    Image: ["Image=docker.io/johly/airtrail:latest"]
  )
end

podman_container "airtrail" do
  config(
    Container: %W{
      Image=airtrail.image
      Pod=web.pod
      User=#{node["calculon"]["data"]["uid"]}
      Environment=HOST=::1
      Environment=SCRIPT_NAME=/airtrail
      Environment=ORIGIN=#{airtrail_base_url}
      Environment=DB_URL=postgres://#{pguser}:#{pgpasswd}@localhost:#{pgport}/#{pgdb}
      Environment=UPLOAD_LOCATION=/app/uploads
      Environment=PORT=#{airtrail_port}
      Environment=INTEGRATIONS_AERO_DATA_BOX_KEY=#{node["calculon"]["airtrail"]["secrets"]["aerodatabox"]}
      Volume=#{airtrail_files}:/app/uploads
      Annotation=run.oci.condition-wait=#{db_service_unit}:healthy
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Joplin Server",
      "After=#{db_service_unit}",
      "Requires=#{db_service_unit}",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

calculon_www_link "Airtrail" do
  category "Apps"
  url airtrail_base_url
end

podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    upstream_port airtrail_port
    oauth2_proxy(
      emails: node["calculon"]["www"]["user_emails"],
      port: 4200
    )
end

systemd_unit "airtrail-backup-files.service" do
  content <<~EOH
[Unit]
Description=Daily Airtrail Files Backup
After=airtrail.service

[Service]
Type=oneshot
ExecStart=/usr/bin/rsync -aAXHv --delete #{airtrail_files}/ #{backup_path}/

[Install]
WantedBy=default.target
  EOH
  action %i(create enable)
end

systemd_unit "airtrail-backup-files.timer" do
  content <<~EOH
[Unit]
Description=Run Airtrail Files Backup Daily

[Timer]
OnCalendar=daily
RandomizedDelaySec=4h
Persistent=true

[Install]
WantedBy=timers.target
  EOH
  action %i(create enable start)
end
