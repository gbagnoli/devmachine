node.override["server"]["chef"]["cron"]["minute"] = "10"

node.override["nodejs"]["repo"] = "https://deb.nodesource.com/node_13.x"
include_recipe "nodejs::nodejs_from_package"
include_recipe "nodejs::npm"

conf = node["quakejs"]
raise Exception, "rconpassword must be set in secrets quakejs.rconpassword" if conf["rconpassword"].nil?

user conf["user"] do
  group conf["group"]
  manage_home true
  uid conf["uid"]
  gid conf["gid"]
  home conf["home"]
end

package "git"

srcdir = "#{conf["home"]}/src"

git srcdir do
  repository conf["repository"]
  revision "master"
  action :sync
  user conf["user"]
  group conf["group"]
  enable_submodules true
  action :checkout
  notifies :install, "npm_package[quakejs]", :immediately
end

npm_package "quakejs" do
  path srcdir
  json true
  user conf["user"]
  action :nothing
end

%w(base base/baseq3).each do |d|
  directory "#{srcdir}/#{d}" do
    user conf["user"]
    group conf["group"]
  end
end
%w(autoexec.cfg server.cfg bots.cfg server.cfg levels.cfg).each do |f|
  template "#{srcdir}/base/baseq3/#{f}" do
    source "#{f}.erb"
    owner conf["user"]
    group conf["group"]
    variables(
      conf: conf
    )
    notifies :restart, "systemd_unit[quakejs.service]"
  end
end


patch = "#{conf["home"]}/src/eula.patch"
cookbook_file patch do
  source "eula.patch"
  owner conf["user"]
  group conf["group"]
  notifies :run, "bash[patch.eula]", :immediately
end

bash "patch.eula" do
 cwd srcdir
 user conf["user"]
 code <<-EOH
  patch -p0 < eula.patch
 EOH
 action :nothing
end


systemd_unit "quakejs.service" do
  content <<~EOH
    [Unit]
    Description=quakejs server

    [Service]
    WorkingDirectory=#{srcdir}
    PrivateUsers=true
    User=#{conf["user"]}
    Group=#{conf["group"]}
    ProtectSystem=full
    ProtectHome=true
    ProtectKernelTunables=true
    ProtectKernelModules=true
    ProtectControlGroups=true
    # We need to accept the EULA first time...
    ExecStart=/usr/bin/node build/ioq3ded.js +set fs_game baseq3 +set dedicated 1 +exec server.cfg +exec levels.cfg
    Restart=on-failure
    RestartSec=60s

    [Install]
    WantedBy=multi-user.target
  EOH
  action %i(create enable start)
end

%w(/var/www /var/www/q3a).each do |d|
  directory d do
    owner "www-data"
    group "www-data"
  end
end

nginx_site "q3a.tigc.eu" do
  template "q3a.tigc.eu.erb"
  variables(
    directory: '/var/www/q3a',
    server_name: 'q3a.tigc.eu'
  )
  action :enable
end
