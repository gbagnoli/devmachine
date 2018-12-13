node.override['syncthing']['users']['giacomo']['hostname'] = 'syncthing.tigc.eu'
node.override['syncthing']['users']['giacomo']['port'] = '8384'

nginx_site 'sync.tigc.eu' do
  template 'syncthing.nginx.erb'
  variables(
    host: '127.0.0.1',
    port: node['syncthing']['users']['giacomo']['port'],
    upstream: 'syncthing',
    server_name: 'sync.tigc.eu',
    oauth2_proxy_port: node['marvin']['oauth2_proxy']['http_port'],
    oauth2_proxy_upstream_port: node['marvin']['oauth2_proxy']['upstream_port']
  )
  action :enable
end
