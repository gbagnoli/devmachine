media_user = node["flexo"]["media"]["username"]
include_recipe "rclone"

if node["putio"].nil? || [node["putio"]["password_encrypted"], node["putio"]["username"]].map(&:nil?).any?
  Chef::Log.error("Skipping rclone/putio config as no username or password has been provided")
  return
end

config = "/etc/rclone/putio.conf"

directory "/etc/rclone" do
  owner media_user
  group "media"
  mode "0550"
end

template config do
  source "rclone.erb"
  owner media_user
  group "media"
  mode "0440"
  variables(
    user: node["putio"]["username"],
    password: node["putio"]["password_encrypted"],
  )
end

root = node["flexo"]["media"]["path"]
destination = node["flexo"]["rclone"]["local_directory"]

template "/usr/local/bin/rclone_putio" do
  mode "0740" # group can read, but they'd have to sudo to execute
  user media_user
  group "media"
  source "putio_rclone.erb"
  variables(
    user: media_user,
    destination: destination,
    config: config,
  )
end

venv = "/var/lib/virtualenvs/3.10/putio_automator"

execute "create_venv_#{venv}" do
  command "/usr/bin/python3 -m virtualenv #{venv}"
  group "media"
  user node["flexo"]["media"]["username"]
  not_if { ::File.directory?(venv) }
  notifies :run, "execute[install_deps_in_venv_#{venv}]", :immediately
end

execute "install_deps_in_venv_#{venv}" do
  command "#{venv}/bin/pip install -U pip wheel setuptools"
  group "media"
  user node["flexo"]["media"]["username"]
  action :nothing
end

["#{venv}/src", "#{root}/tmp", "#{root}/torrents"].each do |d|
  directory d do
    group "media"
    owner media_user
    mode "0750"
  end
end

git "#{venv}/src/putio_automator" do
  repository "https://github.com/gbagnoli/putio-automator.git"
  action :sync
  revision "develop"
  checkout_branch "develop"
  user media_user
  notifies :run, "bash[install_putio_automator]", :immediately
end

bash "install_putio_automator" do
  action :nothing
  cwd venv
  code <<-EOH
    usermod -s /bin/bash #{media_user}
    sudo -i -u #{media_user} #{venv}/bin/pip install -e #{venv}/src/putio_automator/
    usermod -s /bin/false #{media_user}
  EOH
end

template "#{venv}/config.py" do
  source "putio_automator.erb"
  user media_user
  group "media"
  variables(
    destination: destination,
    token: node["putio"]["token"],
    incomplete: "#{root}/tmp",
    torrents: "#{root}/torrents",
  )
end

template "/usr/local/bin/putio" do
  source "putio_automator_bin.erb"
  user media_user
  group "media"
  mode "0740"
  variables(
    venv: venv,
  )
end

template "/usr/local/bin/putio_groom_swipe" do
  source "putio_groom_swipe.erb"
  user media_user
  group "media"
  mode "0740"
  variables(
    putio: "/usr/local/bin/putio",
    rclone: "/usr/local/bin/rclone_putio",
    user: media_user,
  )
end

directory "/var/log/putio" do
  owner media_user
  group "media"
  mode "0755"
end

cron_d "putio-sync" do
  minute "*/5"
  user media_user
  home venv
  environment(
    "USER" => media_user,
  )
  command "/usr/local/bin/putio_groom_swipe 2>&1 >> /var/log/putio/sync.log"
end

# daily, we run a sync so we clear up all renamed files
cron_d "putio-sync-daily" do
  minute "17"
  hour "4"
  user media_user
  home venv
  environment(
    "WAIT" => "21600",
    "USER" => media_user,
  )
  command "/usr/local/bin/rclone_putio sync 2>&1 >> /var/log/putio/sync.log"
end

logrotate_app "putio-sync" do
  path "/var/log/putio/sync.log"
  frequency "daily"
  rotate 30
  create "644 #{media_user} media"
end

systemd_unit "putio-watcher.service" do
  content <<~EOU
    [Unit]
    Description=Putio torrent watcher
    Wants=network-online.target
    After=network-online.target

    [Service]
    User=#{node["flexo"]["media"]["username"]}
    Group=media
    ExecStart=#{venv}/bin/putio torrents watch --parent-id #{node["flexo"]["putio"]["watcher_parent_id"]}
    WorkingDirectory=#{venv}

    [Install]
    WantedBy=multi-user.target
  EOU
  action %i(create enable start)
end
