joplin_port=22_300
domain = node["calculon"]["www"]["notes_domain"]
joplin_base_url = "https://#{domain}/"
joplin_base_url = "https://#{node["calculon"]["www"]["domain"]}/joplin" if domain.nil?

# database
pgport = 5959
pgdb = "joplin"
pguser = "joplin"
pgpasswd = node["calculon"]["joplin"]["secrets"]["pgpasswd"]
if pgpasswd.nil?
  Chef::Log.info('calculon: missing secrets for postgresql')
  raise
end

calculon_postgresql "joplin" do
  port pgport
  user pguser
  password pgpasswd
  podman_pod "web.pod"
end

db_service_unit = "postgresql-joplin.service"

podman_image "joplin" do
  config(
    Image: ["Image=docker.io/joplin/server:latest"],
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
      Annotation=run.oci.condition-wait=#{db_service_unit}:healthy
      Environment=MAX_TIME_DRIFT=0
      Volume=/etc/localtime:/etc/localtime:ro
    } + mail_config,
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
