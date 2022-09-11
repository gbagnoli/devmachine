Chef::DSL::Recipe.include Flexo::RandomPassword

package "install_deps_for_media" do
  package_name %w(git unrar curl sqlite3)
end

node.override["nodejs"]["repo"] = "https://deb.nodesource.com/node_18.x"
include_recipe "nodejs::nodejs_from_package"
include_recipe "nodejs::npm"

users = node["server"]["users"].reject { |_, v| v["delete"] }.keys.dup
users << "plex"

group "media" do
  gid node["flexo"]["media"]["gid"]
  members users
  append true
end

package "acl"

media_d = node["flexo"]["media"]["path"]

user node["flexo"]["media"]["username"] do
  uid node["flexo"]["media"]["uid"]
  gid "media"
  system true
  shell "/bin/false"
end

directory media_d do
  group "media"
  owner node["flexo"]["media"]["username"]
  mode "2775"
end

directory "#{media_d}/downloads" do
  group "media"
  owner node["flexo"]["media"]["username"]
  mode "2775"
end

execute "setfacl_#{media_d}" do
  command "setfacl -R -d -m g::rwx -m o::rx #{media_d}"
  user "root"
  not_if "getfacl #{media_d} 2>/dev/null | grep 'default:' -q"
end

venv_base_path = "/var/lib/virtualenvs"
{"3.10" => "python3"}.each do |version, pfx|
  package "python#{version}" do
    package_name ["python#{version}", "python#{version}-dev",
                  "#{pfx}-wheel", "#{pfx}-pip", "#{pfx}-setuptools",
                  "#{pfx}-virtualenv"]
  end

  virtualenv_path = "#{venv_base_path}/#{version}"
  directory virtualenv_path do
    recursive true
    group "media"
    mode "0775"
  end
end

# rubocop:disable Metrics/BlockLength
{
  "sickchill" => {
    repo: "https://github.com/SickChill/SickChill.git",
    command: "%<venv>s/bin/python %<venv>s/src/%<app>s/SickChill.py --nolaunch " \
             "-q --datadir=%<datadir>s -p %<port>s",
    config_fname: "config.ini",
    py_packages: [],
    py_runtime: "3.10",
    dir: "series",
    enabled: true,
  }
}.each do |app, config|
  next unless config[:enabled]

  virtualenv_path = "#{venv_base_path}/#{config[:py_runtime]}"
  venv = "#{virtualenv_path}/#{app}"
  datadir = "/var/lib/#{app}"
  root_d = "#{media_d}/#{config[:dir]}"
  download_d = "#{media_d}/downloads/#{config[:dir]}"

  directory root_d do
    group "media"
    owner node["flexo"]["media"]["username"]
    mode "2775"
  end

  attrs = node["flexo"]["media"][app] || {}
  command = config[:command] % { # rubocop: disable Style/FormatString
    venv: venv,
    app: app,
    datadir: datadir,
    port: attrs["port"],
  }

  execute "create_venv[#{venv}]" do
    command "/usr/bin/python3 -m virtualenv -p /usr/bin/python#{config[:py_runtime]} #{venv}"
    group "media"
    user node["flexo"]["media"]["username"]
    not_if { ::File.directory?(venv) }
    notifies :run, "execute[install_deps_in_venv_#{venv}]", :immediately
  end

  vpip = "#{venv}/bin/pip"

  execute "install_deps_in_venv_#{venv}" do
    command "#{vpip} install -U pip wheel setuptools"
    group "media"
    user node["flexo"]["media"]["username"]
    action :nothing
  end

  config[:py_packages].each do |pkg|
    execute "install #{pkg} in #{venv}" do
      command "#{vpip} install -U #{pkg}"
      group "media"
      user node["flexo"]["media"]["username"]
      action :nothing
      subscribes :run, "execute[install_deps_in_venv_#{venv}]", :immediately
    end
  end

  directory "#{venv}/src" do
    group "media"
    owner node["flexo"]["media"]["username"]
    mode "0750"
  end

  git "#{venv}/src/#{app}" do
    repository config[:repo]
    action :sync
    revision "master"
    checkout_branch "master"
    user node["flexo"]["media"]["username"]
    notifies :run, "bash[install #{app}]", :immediately
    notifies :restart, "systemd_unit[#{app}.service]", :delayed
  end

  bash "install #{app}" do
    action :nothing
    cwd venv
    code <<-EOH
      usermod -s /bin/bash #{node["flexo"]["media"]["username"]}
      sudo -i -u #{node["flexo"]["media"]["username"]} #{venv}/bin/pip install -e #{venv}/src/#{app}
      usermod -s /bin/false #{node["flexo"]["media"]["username"]}
    EOH
  end

  directory datadir do
    owner node["flexo"]["media"]["username"]
    group "media"
    mode "0750"
    recursive true
  end

  cookie_secret = random_password
  encryption_secret = random_password
  api_key = random_password

  template "#{datadir}/#{config[:config_fname]}" do
    owner node["flexo"]["media"]["username"]
    group "media"
    mode "0640"
    source "#{app}-config.ini.erb"
    action :create_if_missing
    variables(
      cookie_secret: cookie_secret,
      encryption_secret: encryption_secret,
      api_key: api_key,
      download_dir: download_d,
      root_dir: root_d,
    )
  end

  systemd_unit "#{app}.service" do
    content <<~EOU
      [Unit]
      Description=#{app}
      Wants=network-online.target
      After=network-online.target

      [Service]
      User=#{node["flexo"]["media"]["username"]}
      Group=media
      ExecStart=#{command}

      [Install]
      WantedBy=multi-user.target
    EOU
    action %i(create enable start)
  end
end
# rubocop:enable Metrics/BlockLength

radarr_root_d = "/var/lib/radarr"
radarr_code_d = "#{radarr_root_d}/Radarr"
radarr_conf_d = "#{radarr_root_d}/conf"
radarr_listen_d = "#{media_d}/downloads/movies"

[radarr_root_d, radarr_code_d, radarr_conf_d, radarr_listen_d].each do |dir|
  directory dir do
    user node["flexo"]["media"]["username"]
    group "media"
    mode '750'
  end
end

remote_file "#{Chef::Config[:file_cache_path]}/radarr.tar.gz" do
  source 'http://radarr.servarr.com/v1/update/master/updatefile?os=linux&runtime=netcore&arch=x64'
  user node["flexo"]["media"]["username"]
  group "media"
  mode '644'
  notifies :run, "execute[unpack_radarr]", :immediately
end

execute "unpack_radarr" do
  action :nothing
  group "media"
  user node["flexo"]["media"]["username"]
  command "tar xzf #{Chef::Config[:file_cache_path]}/radarr.tar.gz -C #{radarr_root_d}"
end

systemd_unit "radarr.service" do
  content <<~EOU
    [Unit]
    Description=Radarr Daemon
    After=syslog.target network.target
    [Service]
    User=#{node["flexo"]["media"]["username"]}
    Group=media
    Type=simple

    ExecStart=#{radarr_code_d}/Radarr -nobrowser -data=#{radarr_conf_d}/
    TimeoutStopSec=20
    KillMode=process
    Restart=on-failure
    [Install]
    WantedBy=multi-user.target
  EOU
  action %i(create enable start)
end

directory "/var/www/" do
  owner "www-data"
  group "www-data"
end

cookbook_file "/var/www/index.html" do
  source "index.html"
  owner "www-data"
  group "www-data"
end

nginx_site "media.tigc.eu" do
  template "media.nginx.erb"
  variables(
    host: "127.0.0.1",
    sickchill_port: node["flexo"]["media"]["sickchill"]["port"],
    couchpotato_port: node["flexo"]["media"]["couchpotato"]["port"],
    radarr_port: node["flexo"]["media"]["radarr"]["port"],
    server_name: "media.tigc.eu",
    oauth2_proxy_port: lazy { node["server"]["oauth2_proxy"]["http_port"] },
    oauth2_proxy_upstream_port: lazy { node["server"]["oauth2_proxy"]["upstream_port"] },
  )
  action :enable
end

include_recipe "flexo::putio"
