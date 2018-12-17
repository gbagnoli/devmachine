deb = "#{Chef::Config[:file_cache_path]}/rclone.deb"

remote_file deb do
  source 'https://downloads.rclone.org/rclone-current-linux-amd64.deb'
  notifies :run, 'execute[install rclone]', :immediately
end

execute 'install rclone' do
  command "dpkg -i #{deb}"
  action :nothing
end

media_user = node['flexo']['media']['username']

directory '/etc/rclone' do
  owner media_user
  group 'media'
  mode '0550'
end

if node['putio'].nil? || [node['putio']['password_encrypted'], node['putio']['username']].map(&:nil?).any?
  Chef::Log.error('Skipping rclone config as no username or password has been provided')
  return
end

config = '/etc/rclone/putio.conf'

template config do
  source 'rclone.erb'
  owner media_user
  group 'media'
  mode '0440'
  variables(
    user: node['putio']['username'],
    password: node['putio']['password_encrypted']
  )
end

root = node['flexo']['media']['path']
destination = node['flexo']['rclone']['local_directory']

template '/usr/local/bin/rclone_putio' do
  mode '0740' # group can read, but they'd have to sudo to execute
  user media_user
  group 'media'
  source 'putio_rclone.erb'
  variables(
    user: media_user,
    destination: destination,
    config: config
  )
end

venv = '/var/lib/virtualenvs/2.7/putio_automator'

python_virtualenv venv do
  pip_version '18.0'
  group 'media'
  user media_user
  python '2.7'
end

["#{venv}/src", "#{root}/tmp", "#{root}/torrents"].each do |d|
  directory d do
    group 'media'
    owner media_user
    mode '0750'
  end
end

git "#{venv}/src/putio_automator" do
  repository 'https://github.com/gbagnoli/putio-automator.git'
  action :sync
  revision 'develop'
  checkout_branch 'develop'
  user media_user
  notifies :run, 'bash[install_putio_automator]', :immediately
end

bash 'install_putio_automator' do
  action :nothing
  cwd venv
  code <<-EOH
    usermod -s /bin/bash #{media_user}
    sudo -i -u #{media_user} #{venv}/bin/pip install -e #{venv}/src/putio_automator/
    usermod -s /bin/false #{media_user}
  EOH
end

template "#{venv}/config.py" do
  source 'putio_automator.erb'
  user media_user
  group 'media'
  variables(
    destination: destination,
    token: node['putio']['token'],
    incomplete: "#{root}/tmp",
    torrents: "#{root}/torrents"
  )
end

template '/usr/local/bin/putio' do
  source 'putio_automator_bin.erb'
  user media_user
  group 'media'
  mode '0740'
  variables(
    venv: venv
  )
end

template '/usr/local/bin/putio_groom_swipe' do
  source 'putio_groom_swipe.erb'
  user media_user
  group 'media'
  mode '0740'
  variables(
    putio: '/usr/local/bin/putio',
    rclone: '/usr/local/bin/rclone_putio',
    user: media_user
  )
end

directory '/var/log/putio' do
  owner media_user
  group 'media'
  mode '0755'
end

cron_d 'putio-sync' do
  minute '*/5'
  user media_user
  home venv
  environment(
    'USER' => media_user
  )
  command '/usr/local/bin/putio_groom_swipe 2>&1 >> /var/log/putio/sync.log'
end

logrotate_app 'putio-sync' do
  path      '/var/log/putio/sync.log'
  frequency 'daily'
  rotate    30
  create    "644 #{media_user} media"
end
