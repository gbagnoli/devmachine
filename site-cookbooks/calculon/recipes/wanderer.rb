web_ext_port = 59000
pocketbase_ext_port = 59001
web_port = 3000
meili_port = 7700
meili_url = "http://localhost:#{meili_port}"
pocketbase_port = 8090
pocketbase_url = "http://localhost:#{pocketbase_port}"
wanderer_root = node["calculon"]["storage"]["paths"]["wanderer"]
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
secrets = node["calculon"]["wanderer"]["secrets"]
domain = node["calculon"]["wanderer"]["domain"]
if secrets.empty?
  Chef::Log.info('calculon: missing secrets for wanderer')
  raise
end
if domain.nil? || domain.empty?
  Chef::Log.info('calculon: missing domain for wanderer')
  raise
end
public_url = "https://#{domain}"
private_url = "http://localhost:#{web_port}"
envfile="/etc/containers/systemd/wanderer.env"

%w{pocketbase_encryption_key meili_master_key email admin_email admin_password}.each do |s|
  if secrets[s].nil? || secrets[s].empty?
    Chef::Log.info("calculon: missing secret calculon.wanderer.#{s} for wanderer")
    raise
  end
end

if secrets["email"]
  email_envs = %w{username password host port sender_name sender_address}.map do |s|
    if secrets["email"][s].nil? || secrets["email"][s].to_s.empty?
      Chef::Log.info("calculon: missing email secret calculon.secrets.wanderer.email.#{s} for wanderer")
      raise
    end
    ["POCKETBASE_SMTP_#{s.upcase}", secrets["email"][s].to_s]
  end.to_h
  email_envs["POCKETBASE_SMTP_ENABLED"] = "true"
else
  email_envs = {}
end

podman_pod "wanderer" do
  config(
    Pod: %W{
      PublishPort=[::]:#{web_ext_port}:#{web_port}/tcp
      PublishPort=#{web_ext_port}:#{web_port}/tcp
      PublishPort=[::]:#{pocketbase_ext_port}:#{pocketbase_port}/tcp
      PublishPort=#{pocketbase_ext_port}:#{pocketbase_port}/tcp
      Network=calculon.network
    }
  )
end

calculon_btrfs_volume wanderer_root do
  owner user
  group group
  mode "2775"
  setfacl true
end

[
  "#{wanderer_root}/uploads",
  "#{wanderer_root}/pocketbase",
  "#{wanderer_root}/meili",
].each do |dir|
  directory dir do
    owner user
    group group
    mode "2755"
  end
end

config = {
  "MEILI_URL" => meili_url,
  "MEILI_MASTER_KEY" => secrets["meili_master_key"],
  "MEILI_NO_ANALYTICS" => "true",
  "POCKETBASE_ENCRYPTION_KEY" => secrets["pocketbase_encryption_key"],
  "ORIGIN" => public_url,
  "PUBLIC_POCKETBASE_URL" => "http://localhost:#{pocketbase_port}",
  "PUBLIC_DISABLE_SIGNUP" => "true",
  "PUBLIC_PRIVATE_INSTANCE" => "true",
  "UPLOAD_FOLDER" => "/app/uploads",
  "UPLOAD_USER" => "",
  "UPLOAD_PASSWORD" => "",
  "PUBLIC_OVERPASS_API_URL" => "https://overpass-api.de",
  "PUBLIC_VALHALLA_URL" => "https://valhalla1.openstreetmap.de",
  "PUBLIC_NOMINATIM_URL" => "https://nominatim.openstreetmap.org",
}.merge(email_envs)

file envfile do
  mode "0400"
  content config.sort.map {|k,v| "#{k.upcase}=#{v}"}.join("\n")
end


search_container = "wanderer-search"
db_container = "wanderer-db"
web_container = "wanderer-web"

search_service = "#{search_container}.service"
db_service = "#{db_container}.service"
web_service = "#{web_container}.service"

podman_container search_container do
  config(
    Container: [
      "Image=docker.io/getmeili/meilisearch:v1.36.0",
      "Pod=wanderer.pod",
      "EnvironmentFile=#{envfile}",
      "Volume=#{wanderer_root}/meili:/meili_data/data.ms",
      "HealthCmd=curl --fail #{meili_url}/health",
      "HealthInterval=15s",
      "HealthRetries=10",
      "HealthStartPeriod=20s",
      "HealthTimeout=10s",
    ],
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Wanderer Meili Search",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

podman_container db_container do
  config(
    Container: [
      "Image=docker.io/flomp/wanderer-db:latest",
      "Annotation=run.oci.condition-wait=#{search_service}:healthy",
      "Pod=wanderer.pod",
      "EnvironmentFile=#{envfile}",
      "Volume=#{wanderer_root}/pocketbase:/pb_data",
      "HealthCmd=/curl --fail #{pocketbase_url}/health",
      "HealthInterval=15s",
      "HealthRetries=10",
      "HealthStartPeriod=20s",
      "HealthTimeout=10s",
    ],
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Wanderer PocketBase db",
      "After=#{search_service}",
      "Requires=#{search_service}",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

podman_container web_container do
  config(
    Container: [
      "Image=docker.io/flomp/wanderer-web:latest",
      "EnvironmentFile=#{envfile}",
      "Annotation=run.oci.condition-wait=#{search_service}:healthy",
      "Annotation=run.oci.condition-wait=#{db_service}:healthy",
      "Pod=wanderer.pod",
      "HealthCmd=curl --fail #{private_url}/",
      "HealthInterval=15s",
      "HealthRetries=10",
      "HealthStartPeriod=20s",
      "HealthTimeout=10s",
      "Volume=#{wanderer_root}/uploads:/app/uploads",
    ],
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Wanderer PocketBase db",
      "After=#{search_service}",
      "Requires=#{search_service}",
      "After=#{db_service}",
      "Requires=#{db_service}",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

[search_service, db_service, web_service].each do |s|
  service s do
    action :start
    subscribes :restart, "file[#{envfile}]"
  end
end

calculon_www_link "Wanderer - Trails" do
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
  "proxy_buffer_size 128k",
  "proxy_buffers 4 256k",
  "proxy_busy_buffers_size 256k",
]

addr4 = node["calculon"]["network"]["containers"]["ipv4"]["addr"]
podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    upgrade true
    upstream_address addr4
    upstream_port web_port
    default_location_force_https true
    default_location_extra_config extra_properties.join(";\n") + ";"
    extra_config <<EOH
      large_client_header_buffers 4 32k;
EOH
end

calculon_www_upstream "/wanderer-admin" do
  upstream_address addr4
  upstream_port pocketbase_ext_port
  upstream_path "/_"
  upgrade true
  title "Wanderer Admin"
  category "Tools"
end

calculon_www_upstream "/api" do
  upstream_address addr4
  upstream_port pocketbase_ext_port
  upstream_path "/api"
  title "Wanderer Api"
  matcher ""
  upgrade true
  nolink true
end

# create the superuser for pocketbase if not created already
#
bash "wanderer_create_admin" do
  creates "#{wanderer_root}/.superuser_created"
  code <<EOH
/usr/bin/podman exec -it #{db_container} /pocketbase superuser upsert "#{secrets["admin_email"]}" "#{secrets["admin_password"]}"|| exit 1
touch #{wanderer_root}/.superuser_created"
EOH
end
