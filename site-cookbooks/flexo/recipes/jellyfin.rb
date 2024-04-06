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
