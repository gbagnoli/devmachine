node.override['syncthing']['users']['giacomo']['hostname'] = 'syncthing.tigc.eu'
node.override['syncthing']['users']['giacomo']['port'] = '8384'

nginx_site 'sync.tigc.eu' do
  template 'syncthing.nginx.erb'
  variables(
    host: '127.0.0.1',
    port: node['syncthing']['users']['giacomo']['port'],
    upstream: 'syncthing',
    server_name: 'sync.tigc.eu',
    oauth2_proxy_port: node['server']['oauth2_proxy']['http_port'],
    oauth2_proxy_upstream_port: node['server']['oauth2_proxy']['upstream_port']
  )
  action :enable
end

include_recipe 'rclone'
conf = node['google_drive'] || {}
token = conf['token']
refresh_token = conf['refresh_token']
expiry = conf['expiry']

if token.nil? || refresh_token.nil? || expiry.nil?
  Chef::Log.error('Skipping rclone config as no token or refresh_token or expiry has been provided')
  return
end

config = "/home/#{node['user']['login']}/.rclone.conf"

template config do
  user node['user']['login']
  group node['user']['group']
  source 'rclone.conf.erb'
  mode '0400'
  action :create_if_missing
  variables(
    token: token,
    refresh_token: refresh_token,
    expiry: expiry
  )
end

sync_d = "/home/#{node['user']['login']}/#{node['marvin']['google_photos_backup']['directory']}"

directory '/var/log/backup_google_photos' do
  owner node['user']['login']
  group node['user']['group']
  mode '0755'
end

log = '/var/log/backup_google_photos/sync.log'

template '/usr/local/bin/backup_google_photos' do
  mode '0755'
  source 'backup_google_photos.erb'
  variables(
    user: node['user']['login'],
    log: log,
    destination: sync_d
  )
end

cron_d 'google_photos_backup' do
  minute '0'
  hour '0'
  user node['user']['login']
  home sync_d
  environment(
    'USER' => node['user']['login']
  )
  command '/usr/local/bin/backup_google_photos 2>&1 > /dev/null'
end

logrotate_app 'google_photos_backup' do
  path log
  frequency 'daily'
  rotate 30
  create "644 #{node['user']['login']} #{node['user']['group']}"
end
