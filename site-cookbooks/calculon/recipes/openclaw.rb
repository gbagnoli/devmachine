user=node["user"]["login"]
uid=node["user"]["uid"]
gid=node["user"]["gid"]
domain = node["calculon"]["www"]["openclaw_domain"]

podman_image "openclaw" do
  config(
    Image: ["Image=ghcr.io/openclaw/openclaw:latest"],
  )
end

conf = node["calculon"]["openclaw"]
ipv4 = conf["ipv4"].empty? ? "" : "#{conf["ipv4"]}:"

directory "/home/#{user}/.openclaw" do
  action :create
  owner user
  group gid
  mode "0700"
end

podman_container "openclaw-#{user}" do
  config(
    Container: %W{
      Image=openclaw.image
      UserNS=keep-id:uid=#{uid},gid=#{gid}
      PublishPort=#{ipv4}#{conf["port"]}:18789
      PublishPort=[#{conf["ipv6"]}]:#{conf["port"]}:18789
      Environment=OPENCLAW_GATEWAY_BIND=::
      Environment=NODE_OPTIONS="--dns-result-order=ipv4first"
      Volume=/etc/localtime:/etc/localtime:ro
      Volume=/home/#{user}/.openclaw:/home/node/.openclaw:Z
      Volume=/home/#{user}/.ssh:/home/node/.ssh:ro,Z
      Volume=/home/#{user}/workspace:/home/node/workspace:Z
      Volume=/home/linuxbrew:/home/linuxbrew:ro,Z
    },
    Service: %w{
      Restart=always
    },
    Unit: [
      "Description=Openclaw getaway",
      "After=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target default.target"
    ]
  )
end

return if domain.nil?

calculon_www_link "Openclaw" do
  category "Apps"
  url "https://#{domain}/"
end

podman_nginx_vhost domain do
  server_name domain
  cloudflare true
  upstream_address "[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]"
  upstream_port conf["port"]
  oauth2_proxy(
    emails: node["calculon"]["openclaw"]["secrets"]["emails"],
    port: 4202
  )
end
