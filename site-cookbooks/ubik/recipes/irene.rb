user = 'irene'
group = 'irene'
home = "/home/#{user}"
uid = 1001
gid = 1001
realname = 'Irene Bagni'

group group do
  gid gid
end

user user do
  group group
  shell '/bin/bash'
  uid uid
  gid gid
  home home
  manage_home true
  comment realname
end

[ "#{home}/Sync",
  "#{home}/Sync/Private"].each do |d|
  directory d do
    mode '0750'
    owner user
    group 'users'
  end
end

file "#{home}/examples.desktop" do
  action :delete
end

syncthing_conf_d = "#{home}/.config/syncthing"
syncthing_conf = "#{syncthing_conf_d}/config.xml"

# configure syncthing
execute 'create syncthing config' do
  command "syncthing --generate #{syncthing_conf_d}"
  user user
  not_if { File.directory? syncthing_conf_d }
  notifies :run, 'bash[fix syncthing config]', :immediately
end

bash 'fix syncthing config' do
  action :nothing
  code <<-EOH
  sed -i -e 's/name="#{node['hostname']}"/name="#{node['hostname']}-irene"/' #{syncthing_conf}
  sed -i -e 's/<address>127.0.0.1:[0-9]*/<address>127.0.0.1:8385/' #{syncthing_conf}
  EOH
end

service 'syncthing@irene' do
  action [:enable, :start]
  provider Chef::Provider::Service::Systemd
end
