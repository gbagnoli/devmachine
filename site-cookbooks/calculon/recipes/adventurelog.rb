frontend_port=58000
server_port=58001
server_url="http://localhost:#{server_port}"

pgport=6060
pgdb="adventurelog"
pguser="adventurelog"
db_service_unit = "postgresql-#{pgdb}.service"
secrets=node["calculon"]["adventurelog"]["secrets"]
if secrets.empty?
  Chef::Log.info('calculon: missing secrets for adventurelog')
  raise
end
domain = node["calculon"]["adventurelog"]["domain"]

%w{pgpasswd secret_key admin_passwd admin_user admin_email google_maps_api_key email}.each do |s|
  if secrets[s].nil? || secrets[s].empty?
    Chef::Log.info("calculon: missing secret calculon.adventurelog.#{s} for adventurelog")
    raise
  end
end

email_secrets = secrets["email"]
%w{host_user host_password host port use_ssl use_tls default_from_email}.each do |s|
  if email_secrets[s].nil? || email_secrets[s].empty?
    Chef::Log.info("calculon: missing email secret calculon.secrets.email.#{s} for adventurelog")
    raise
  end
end

calculon_postgresql pgdb do
  port pgport
  user pguser
  password pgpasswd
  podman_pod "web.pod"
  image "docker.io/postgis/postgis:16-3.5"
  dbenv "POSTGRES_DB"
end

email_config = %W{
  Environment=EMAIL_BACKEND=email
  Environment=DEFAULT_FROM_EMAIL=#{email_secrets["default_from_email"]}
} + %w{host_user host_password host port use_ssl use_tls}.sort.map do |k|
  "Environment=EMAIL_#{k.upcase}=#{email_secrets[k]}"
end

podman_image "adventurelog-server" do
  config(
    Image: ["Image=ghcr.io/seanmorley15/adventurelog-backend:latest"],
  )
end

podman_container "adventurelog-server" do
  config(
    Container: %W{
      Image=adventurelog-server.image
      Pod=web.pod
      Environment=PGHOST=::1
      Environment=PGPORT=#{pgport}
      Environment=POSTGRES_DB=#{pgdb}
      Environment=POSTGRES_USER=#{pguser}
      Environment=POSTGRES_PASSWORD=#{pgpasswd}
      Environment=APP_PORT=#{joplin_port}
      Environment=APP_BASE_URL=#{joplin_base_url}
      Annotation=run.oci.condition-wait=#{db_service_unit}:healthy
      Environment=MAX_TIME_DRIFT=0
      Volume=/etc/localtime:/etc/localtime:ro
    } + email_config,
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
