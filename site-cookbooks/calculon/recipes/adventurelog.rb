frontend_port=58000
backend_port=58001

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
public_url = "https://#{domain}"
envfile="/etc/containers/systemd/adventurelog-server.env"

%w{pgpasswd secret_key admin_passwd admin_user admin_email google_maps_api_key email}.each do |s|
  if secrets[s].nil? || secrets[s].empty?
    Chef::Log.info("calculon: missing secret calculon.adventurelog.#{s} for adventurelog")
    raise
  end
end

if secrets["email"]
  email_envs = %w{host_user host_password host port use_ssl use_tls default_from_email}.map do |s|
    if secrets["email"][s].nil? || secrets["email"][s].to_s.empty?
      Chef::Log.info("calculon: missing email secret calculon.secrets.email.#{s} for adventurelog")
      raise
    end
    key = s == "default_from_email" ? s.upcase : "EMAIL_#{s.upcase}"
    [key, secrets["email"][s].to_s]
  end.to_h
  email_envs["EMAIL_BACKEND"] = "email"
else
  email_envs = {}
end

podman_pod "adventurelog" do
  config(
    Pod: %W{
      PublishPort=[::]:#{backend_port}:8000/tcp
      PublishPort=#{backend_port}:8000/tcp
      PublishPort=[::]:#{frontend_port}:3000/tcp
      PublishPort=#{frontend_port}:3000/tcp
      Network=calculon.network
    }
  )
end

calculon_postgresql pgdb do
  port pgport
  user pguser
  password secrets["pgpasswd"]
  podman_pod "adventurelog.pod"
  image "docker.io/postgis/postgis:16-3.5"
  dbenv "POSTGRES_DB"
end

config = {
  "BACKEND_PORT" => backend_port,
  "FRONTEND_PORT" => frontend_port,
  "BODY_SIZE_LIMIT" => "Infinity",
  "DJANGO_ADMIN_EMAIL" => secrets["admin_email"],
  "DJANGO_ADMIN_PASSWORD" => secrets["admin_passwd"],
  "DJANGO_ADMIN_USERNAME" => secrets["admin_user"],
  "SECRET_KEY" => secrets["secret_key"],
  "GOOGLE_MAPS_API_KEY" => secrets["google_maps_api_key"],
  "DISABLE_REGISTRATION" => "True",
  # postgresql
  "PGHOST" => "::1",
  "PGPORT" => pgport,
  "POSTGRES_DB" => pgdb,
  "POSTGRES_PASSWORD" => secrets["pgpasswd"],
  "POSTGRES_USER" => pguser,
  # domains
  "CSRF_TRUSTED_ORIGINS" => public_url,
  "FRONTEND_URL" => public_url,
  "ORIGIN" => public_url,
  "PUBLIC_SERVER_URL" => "http://localhost:8000",
  "PUBLIC_URL" => public_url,
}.merge(email_envs)

file envfile do
  mode "0400"
  content config.sort.map {|k,v| "#{k.upcase}=#{v}"}.join("\n")
end

podman_image "adventurelog-server" do
  config(
    Image: ["Image=ghcr.io/seanmorley15/adventurelog-backend:latest"],
  )
end
podman_image "adventurelog-frontend" do
  config(
    Image: ["Image=ghcr.io/seanmorley15/adventurelog-frontend:latest"],
  )
end

podman_container "adventurelog-server" do
  config(
    Container: %W{
      Image=adventurelog-server.image
      Pod=adventurelog.pod
      EnvironmentFile=#{envfile}
      Annotation=run.oci.condition-wait=#{db_service_unit}:healthy
      Volume=/etc/localtime:/etc/localtime:ro
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Adventurelog Backend Server",
      "After=#{db_service_unit}",
      "Requires=#{db_service_unit}",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end
podman_container "adventurelog-frontend" do
  config(
    Container: %W{
      Image=adventurelog-frontend.image
      Pod=adventurelog.pod
      EnvironmentFile=#{envfile}
      Volume=/etc/localtime:/etc/localtime:ro
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=AdventureLog Frontend Server",
      "After=adventurelog-server.service",
      "Requires=adventurelog-server.service"
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

calculon_www_link "AdventureLog" do
  category "Apps"
  url public_url
end

extra_properties = [
  "client_max_body_size 200M",
  "client_body_buffer_size 128k",
  "proxy_read_timeout 300",
  "proxy_send_timeout 300",
  "proxy_connect_timeout 300",
  "send_timeout 300",
]

addr6 = "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
addr4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]

podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    upstream_paths({
      "/" => {
        "upgrade" => "$http_connection",
        "extra_properties" => extra_properties,
        "upstream" => "http://#{addr4}:#{frontend_port}",
        "force_https" => true,
        "matcher" => "",
      },
      "^/(media|admin|static|accounts)" => {
        "matcher" => "~",
        "upgrade" => "$http_connection",
        "extra_properties" => extra_properties,
        "upstream" => "http://#{addr6}:#{backend_port}",
        "force_https" => true,
      }
    })
    upgrade true
    oauth2_proxy(
      emails: node["calculon"]["www"]["user_emails"],
      port: 4203,
      pass_auth: true
    )
    act_as_upstream 4204
    default_location_force_https true
end
