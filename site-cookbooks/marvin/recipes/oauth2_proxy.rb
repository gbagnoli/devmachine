include_recipe 'server::oauth2_proxy'

if node['marvin']['oauth2_proxy']['client-secret'].nil?
  Chef::Log.error('Skipping oauth2_proxy configuration as no attrs for secrets were found')
  return
end

conf = node['marvin']['oauth2_proxy']

user 'oauth2proxy' do
  system true
  shell '/bin/false'
  group 'nogroup'
end

file '/etc/oauth2_proxy/tigc.emails.txt' do
  content conf['authenticated_emails'].sort.join("\n")
  mode 0o400
  owner 'oauth2proxy'
  group 'nogroup'
  notifies :restart, 'service[oauth2_proxy-tigc]', :immediately
end

oauth2_proxy_site 'tigc' do
  auth_provider conf['auth_provider']
  http_address  "127.0.0.1:#{conf['http_port']}"
  upstreams ["http://127.0.0.1:#{conf['upstream_port']}/"]
  redirect_url conf['redirect-url']
  authenticated_emails_file '/etc/oauth2_proxy/tigc.emails.txt'
  cookie_secret conf['cookie-secret']
  client_id conf['client-id']
  client_secret conf['client-secret']
end

directory '/etc/systemd/system/oauth2_proxy-tigc.service.d'

file '/etc/systemd/system/oauth2_proxy-tigc.service.d/override.conf' do
  content <<~HEREDOC
    [Service]
    User=oauth2proxy
    Group=nogroup
  HEREDOC
  notifies :run, 'execute[oauth2proxy-reload-tigc]', :immediately
end

execute 'oauth2proxy-reload-tigc' do
  action :nothing
  notifies :restart, 'service[oauth2_proxy-tigc]'
  command 'systemctl daemon-reload'
end

service 'oauth2_proxy-tigc'
