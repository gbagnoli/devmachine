conf = node["cloudflare_ddns"]

podman_image "cloudflare-ddns" do
  config(
    Image: ["Image=docker.io/timothyjmiller/cloudflare-ddns:latest"],
  )
end

ohai 'etc' do
  plugin 'etc'
  action :nothing
end

group "cfddns" do
  system true
end

user "cfddns" do
  group "cfddns"
  system true
  shell "/bin/nologin"
  notifies :reload, 'ohai[etc]', :immediately
end

config_file = "#{conf["directory"]}/config.json"

directory conf["directory"] do
  owner "cfddns"
  group "cfddns"
  mode "0700"
end

file config_file do
  owner "cfddns"
  group "cfddns"
  mode "0600"
  content(
    lazy { conf["config"].to_json }
  )
  notifies :restart, "service[cloudflare-ddns]"
end

podman_container "cloudflare-ddns" do
  config(lazy do
    {
      Container: %W{
        Image=cloudflare-ddns.image
        Environment=PUID=#{node["etc"]["passwd"]["cfddns"]["uid"]}
        Environment=PGID=#{node["etc"]["passwd"]["cfddns"]["gid"]}
        Volume=#{config_file}:/config.json
      },
      Unit: [
        "Description=Update cloudflare dynamic DNS",
        "After=network-online.target",
      ],
      Install: [
        "WantedBy=multi-user.target default.target"
      ]
    }
  end)
end

service "cloudflare-ddns" do
  action %i(enable start)
end
