node.override['oauth2_proxy']['install_url'] = 'https://github.com/bitly/oauth2_proxy/releases/download/v2.2/oauth2_proxy-2.2.0.linux-amd64.go1.8.1.tar.gz'
node.override['oauth2_proxy']['checksum'] = '1c16698ed0c85aa47aeb80e608f723835d9d1a8b98bd9ae36a514826b3acce56'
node.override['oauth2_proxy']['install_path'] = '/usr/local/oauth2_proxy'
include_recipe 'oauth2_proxy::install'

if node['server']['oauth2_proxy']['client-secret'].nil?
  Chef::Log.error('Skipping oauth2_proxy configuration as no attrs for secrets were found')
  return
end

conf = node['server']['oauth2_proxy']
instance = conf['instance_name']

user 'oauth2proxy' do
  system true
  shell '/bin/false'
  group 'nogroup'
end

file "/etc/oauth2_proxy/#{instance}.emails.txt" do
  content conf['authenticated_emails'].sort.join("\n")
  mode 0o400
  owner 'oauth2proxy'
  group 'nogroup'
  notifies :restart, "service[oauth2_proxy-#{instance}]"
end

oauth2_proxy_site instance do
  auth_provider conf['auth_provider']
  http_address  "127.0.0.1:#{conf['http_port']}"
  upstreams ["http://127.0.0.1:#{conf['upstream_port']}/"]
  redirect_url conf['redirect-url']
  authenticated_emails_file "/etc/oauth2_proxy/#{instance}.emails.txt"
  cookie_secret conf['cookie-secret']
  client_id conf['client-id']
  client_secret conf['client-secret']
end

# create an override file to set the username, cookbook makes the service run as root
directory "/etc/systemd/system/oauth2_proxy-#{instance}.service.d"

file "/etc/systemd/system/oauth2_proxy-#{instance}.service.d/override.conf" do
  content <<~HEREDOC
    [Service]
    User=oauth2proxy
    Group=nogroup
  HEREDOC
  notifies :run, "execute[oauth2proxy-reload-#{instance}]", :immediately
end

execute "oauth2proxy-reload-#{instance}" do
  action :nothing
  notifies :restart, "service[oauth2_proxy-#{instance}]"
  command 'systemctl daemon-reload'
end

service "oauth2_proxy-#{instance}"
