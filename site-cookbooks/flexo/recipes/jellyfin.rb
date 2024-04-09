Chef::DSL::Recipe.include Flexo::Tdarr
tdarr_version, tdarr_server_url, tdarr_node_url = tdarr_urls

tdarr_root = "/var/lib/tdarr"

directory tdarr_root

package "tdarr_deps" do
  package_name %w{mkvtoolnix libtesseract-dev handbrake-cli}
end

%w[configs logs cache cache/series cache/movies].each do |dir|
  directory "#{tdarr_root}/#{dir}" do
    owner node["flexo"]["media"]["username"]
    group "media"
    mode "0755"
  end
end

{server: tdarr_server_url, node: tdarr_node_url}.each do |component, url|
  install_dir = "/var/lib/tdarr/#{component}"
  zip = "#{Chef::Config[:file_cache_path]}/tdarr_#{component}_#{tdarr_version}.zip"
  service = "tdarr-#{component}"

  directory install_dir do
    mode "0755"
    owner "root"
    group "root"
    recursive true
    action :nothing
  end

  directory "#{install_dir}/Tdarr" do
    mode "0755"
    owner node["flexo"]["media"]["username"]
    group "media"
    recursive true
    action :nothing
  end

  remote_file zip do
    source url
    action :create_if_missing
    notifies :run, "execute[install tdarr #{component}]", :immediately
  end

  execute "install tdarr #{component}" do
    cwd install_dir
    command "unzip -o #{zip}"
    notifies :delete, "directory[#{install_dir}]", :before
    notifies :create, "directory[#{install_dir}]", :before
    notifies :create, "directory[#{install_dir}/Tdarr]", :before
    notifies :restart, "service[#{service}]"
    action :nothing
  end
end

directory "#{tdarr_root}/node/assets/app/plugins" do
  owner node["flexo"]["media"]["username"]
  group "media"
  mode "0755"
end

systemd_unit "tdarr-server.service" do
  action :create
  content <<EOH
  [Unit]
  Description=Tdarr Server Daemon
  After=network.target

  [Service]
  User=#{node["flexo"]["media"]["username"]}
  Group=media

  Type=simple
  WorkingDirectory=#{tdarr_root}/server
  ExecStart=#{tdarr_root}/server/Tdarr_Server
  TimeoutStopSec=20
  KillMode=control-group
  Restart=on-failure

  [Install]
  WantedBy=multi-user.target
EOH
end

systemd_unit "tdarr-node.service" do
  action :create
  content <<EOH
  [Unit]
  Description=Tdarr Node Daemon
  After=network.target
  Requires=tdarr-server.service

  [Service]
  User=#{node["flexo"]["media"]["username"]}
  Group=media

  Type=simple
  WorkingDirectory=#{tdarr_root}/node
  ExecStart=#{tdarr_root}/node/Tdarr_Node
  TimeoutStopSec=20
  KillMode=process
  Restart=on-failure

  [Install]
  WantedBy=multi-user.target
EOH
end

%i{server node}.each do |component|
  service "tdarr-#{component}" do
    action %i{enable start}
  end
end


apt_repository 'jellyfin' do
  uri "https://repo.jellyfin.org/#{node["platform"]}"
  components ["main"]
  arch "amd64"
  key "https://repo.jellyfin.org/jellyfin_team.gpg.key"
end

package "jellyfin" do
  action :upgrade
end

package "intel-opencl-icd"

media_d = node["flexo"]["media"]["path"]
jellyfin_d = "#{media_d}/jellyfin"
cache_d = "#{jellyfin_d}/cache"
logs_d = "#{jellyfin_d}/logs"
data_d = "#{jellyfin_d}/data"
config_d = "#{jellyfin_d}/config"

dirs = [jellyfin_d, cache_d, logs_d, data_d, config_d]

dirs.each do |dir|
  directory dir do
    group "media"
    owner node["flexo"]["media"]["username"]
    mode "2775"
  end
end

template "/etc/systemd/system/jellyfin.service.d/jellyfin.service.conf" do
  source "jellyfin.systemd.dropin.erb"
  variables(
    user: node["flexo"]["media"]["username"],
    group: "media",
    working_directory: jellyfin_d,
  )
  notifies :run, "execute[reload-systemd-jellyfin]", :immediately
  notifies :restart, "service[jellyfin]", :delayed
end

execute "reload-systemd-jellyfin" do
  action :nothing
  command "systemctl daemon-reload"
end

template "/etc/default/jellyfin" do
  source "jellyfin.environment.erb"
  notifies :restart, "service[jellyfin]", :delayed
  variables(
    cache_d: cache_d,
    logs_d: logs_d,
    data_d: data_d,
    config_d: config_d,
  )
end

template "/etc/sudoers.d/jellyfin-sudoers" do
  source "jellyfin.sudoers.erb"
  notifies :restart, "service[jellyfin]", :delayed
  mode "0440"
  owner "root"
  group "root"
  variables(
    user: node["flexo"]["media"]["username"]
  )
end

service "jellyfin" do
  action %i[enable start]
end
