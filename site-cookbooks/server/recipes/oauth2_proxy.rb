include_recipe "oauth2_proxy::install"

if node["server"]["oauth2_proxy"]["client-secret"].nil?
  Chef::Log.error("Skipping oauth2_proxy configuration as no attrs for secrets were found")
  return
end

conf = node["server"]["oauth2_proxy"]
instance = conf["instance_name"]

user "oauth2proxy" do
  system true
  shell "/bin/false"
  group "nogroup"
end

file "/etc/oauth2_proxy/#{instance}.emails.txt" do
  content conf["authenticated_emails"].sort.join("\n")
  mode 0o400
  owner "oauth2proxy"
  group "nogroup"
  notifies :restart, "service[oauth2_proxy-#{instance}]"
end

oauth2_proxy_site instance do
  auth_provider conf["auth_provider"]
  http_address "127.0.0.1:#{conf["http_port"]}"
  upstreams ["http://127.0.0.1:#{conf["upstream_port"]}/"]
  redirect_url conf["redirect-url"]
  authenticated_emails_file "/etc/oauth2_proxy/#{instance}.emails.txt"
  cookie_secret conf["cookie-secret"]
  client_id conf["client-id"]
  client_secret conf["client-secret"]
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
  command "systemctl daemon-reload"
end

service "oauth2_proxy-#{instance}"
