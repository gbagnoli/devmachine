user=node["user"]["login"]
group=node["user"]["group"]
uid=node["user"]["uid"]
gid=node["user"]["gid"]
domain = node["calculon"]["www"]["openclaw_domain"]

podman_image "openclaw" do
  action :delete
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

extra_config = {".config/gh" => "ro", ".gitconfig" => "ro", ".ssh" => "ro", "workspace" => "rw", ".config/gogcli" => "ro"}.map do |vol, access|
  local_path="/home/#{user}/#{vol}"
  remote_path="/home/node/#{vol}"
  access = access == "rw" ? "" : "ro,"
  if File.exist?(local_path)
    "Volume=#{local_path}:#{remote_path}:#{access}Z"
  end
end.compact

if File.exist?("home/linuxbrew/.linuxbrew")
  extra_config << "Volume=/home/linuxbrew:/home/linuxbrew:ro,Z"
  extra_config << 'Environment=PATH="/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"'
end

gogcli_secret = node["calculon"]["openclaw"]["secrets"]["gog_keyring_password"]
if !gogcli_secret.nil? && !gogcli_secret.empty?
  extra_config << "Environment=GOG_KEYRING_PASSWORD=#{gogcli_secret}"
end

podman_container "openclaw-#{user}" do
  config(
    Container: %W{
      Image=ghcr.io/openclaw/openclaw:latest
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
      Volume=/home/#{user}/.openclaw:/home/node/.openclaw:Z
    } + extra_config,
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
