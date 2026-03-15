gokapi_port = 53842
gokapi_root = node["calculon"]["storage"]["paths"]["gokapi"]
gokapi_data = "#{gokapi_root}/files"
gokapi_config = "#{gokapi_root}/config"
user = node["calculon"]["data"]["username"]
group = node["calculon"]["data"]["group"]
uid = node["calculon"]["data"]["uid"]
gid = node["calculon"]["data"]["gid"]

calculon_btrfs_volume gokapi_root do
  owner user
  group group
  mode "2775"
  setfacl true
end

[
  gokapi_data,
  gokapi_config
].each do |dir|
  directory "#{dir}" do
    owner user
    group group
    mode "2755"
  end
end

secrets = node["calculon"]["gokapi"]["secrets"].to_h
%w{public_name domain admin_username}.each do |s|
  if secrets[s].nil?
    Chef::Log.info("calculon: missing secret #{s} for gokapi")
    raise
  end
end
server_url = "https://#{secrets["domain"]}"

podman_image "gokapi" do
  config(
    Image: ["Image=docker.io/f0rc3/gokapi"],
  )
end

template "#{gokapi_config}/config.json" do
  source "gokapi_config.json.erb"
  variables(
    port: gokapi_port,
    public_name: secrets["public_name"],
    server_url: server_url,
    redirect_url: "#{server_url}/admin",
    admin_username: secrets["admin_username"]
  )
  user user
  group group
  mode "0600"
end

podman_container "gokapi" do
  config(
    Container: %W{
      Image=gokapi.image
      Pod=web.pod
      User=#{uid}
      Group=#{gid}
      Environment=TZ=Europe/Madrid
      Environment=GOKAPI_ADMIN_USER=#{secrets["admin_username"]}
      Volume=#{gokapi_data}:/app/data
      Volume=#{gokapi_config}:/app/config
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Gokapi file share",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

calculon_www_link "Send files" do
  category "Apps"
  url server_url
end

podman_nginx_vhost secrets["domain"] do
    server_name secrets["domain"]
    cloudflare true
    upstream_port gokapi_port
    oauth2_proxy(
      emails: node["calculon"]["www"]["user_emails"],
      port: 4201,
      pass_auth: true
    )
end
