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

config = "/home/#{node['user']['login']}/.rclone.conf"
file config do
  action :delete
end


directory '/var/log/backup_google_photos' do
  action :delete
  recursive true
end

file '/usr/local/bin/backup_google_photos' do
  action :delete
end

sync_d = "/home/#{node['user']['login']}/#{node['marvin']['google_photos_backup']['directory']}"
cron_d 'google_photos_backup' do
  minute '0'
  hour '0'
  user node['user']['login']
  home sync_d
  environment(
    'USER' => node['user']['login']
  )
  command '/usr/local/bin/backup_google_photos 2>&1 > /dev/null'
  action :delete
end

logrotate_app 'google_photos_backup' do
  path '/var/log/backup_google_photos/sync.log'
  frequency 'daily'
  rotate 30
  create "644 #{node['user']['login']} #{node['user']['group']}"
  action :disable
end
