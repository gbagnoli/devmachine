user=node["user"]["login"]
group=node["user"]["group"]
uid=node["user"]["uid"]
gid=node["user"]["gid"]
domain = node["calculon"]["www"]["openclaw_domain"]

podman_image "openclaw" do
  config(
    Image: ["Image=ghcr.io/openclaw/openclaw:latest"],
  )
end

directory "/home/#{user}/.openclaw" do
  action :create
  owner user
  group group
  mode "0700"
end

port = node["calculon"]["openclaw"]["port"]

podman_container "openclaw-#{user}" do
  config(
    Container: %W{
      Image=openclaw.image
      UIDMap=0:100000:1000
      UIDMap=1000:#{uid}:1
      UIDMap=1001:101001:64535
      GIDMap=0:100000:1000
      GIDMap=1000:#{gid}:1
      GIDMap=1001:101001:64535
      User=1000:1000
      PublishPort=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}:#{port}:18789
      PublishPort=[#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}]:#{port}:18789
      Environment=OPENCLAW_GATEWAY_BIND=::
      Environment=NODE_OPTIONS="--dns-result-order=ipv4first"
      Volume=/etc/localtime:/etc/localtime:ro
      Volume=/home/#{user}/.config/gh:/home/node/.config/gh:ro,Z
      Volume=/home/#{user}/.gitconfig:/home/node/.gitconfig:ro,Z
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
  upstream_address node["calculon"]["network"]["containers"]["ipv4"]["addr"]
  upstream_port port
  oauth2_proxy(
    emails: node["calculon"]["openclaw"]["secrets"]["emails"],
    port: 4202
  )
end
