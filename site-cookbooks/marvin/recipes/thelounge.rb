include_recipe "marvin::znc"
node.override["nodejs"]["repo"] = "https://deb.nodesource.com/node_18.x"
include_recipe "nodejs::nodejs_from_package"
include_recipe "nodejs::npm"

npm_package "thelounge" do
  options ["--unsafe-perm"]
end

group "thelounge" do
  gid node["marvin"]["thelounge"]["groupid"]
end

user "thelounge" do
  uid node["marvin"]["thelounge"]["userid"]
  gid "thelounge"
  system true
  shell "/bin/false"
  home node["marvin"]["thelounge"]["home"]
end

directory node["marvin"]["thelounge"]["home"] do
  owner "thelounge"
  group "thelounge"
  mode "750"
end

template "#{node["marvin"]["thelounge"]["home"]}/config.js" do
  owner "thelounge"
  group "thelounge"
  source "thelounge_config.js.erb"
  variables(
    home: node["marvin"]["thelounge"]["home"],
    port: node["marvin"]["thelounge"]["port"],
  )
end

systemd_unit "thelounge.service" do
  content <<~EOU
    [Unit]
    Description=Run thelounge irc client

    [Service]
    Environment = 'THELOUNGE_HOME=#{node["marvin"]["thelounge"]["home"]}'
    ExecStart = /usr/bin/thelounge start
    User=thelounge

    [Install]
    WantedBy = multi-user.target
  EOU
  action %i(create enable start)
end

nginx_site "thelounge.tigc.eu" do
  template "nginx.erb"
  variables(
    host: "127.0.0.1",
    port: node["marvin"]["thelounge"]["port"],
    upstream: "thelounge",
    server_name: node["marvin"]["thelounge"]["server_name"],
    location: "/",
    upgrade: true,
  )
  action :enable
end
