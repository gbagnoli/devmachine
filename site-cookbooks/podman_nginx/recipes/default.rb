include_recipe "podman_nginx::default"
conf = node["podman"]["nginx"]

package "nginx" do
  action :purge
end

www = conf["path"]
user = conf["user"]
group = conf["group"]
uid = conf["uid"]
gid = conf["gid"]

# we need to make sure the gid and uid are "free"
# if they are not, let's free them
execute "free_uid_for_nginx" do
  command "usermod -u 9#{uid} $(getent passwd #{uid} | cut -d':' -f 1)"
  not_if "getent passwd #{uid} | cut -d':' -f 1 | grep -q #{user}"
  notifies :request_reboot, "reboot[nginx_user]", :immediately
end

execute "free_guid_for_nginx" do
  command "groupmod -g 9#{gid} $(getent group #{gid} | cut -d':' -f 1)"
  not_if "getent group #{gid} | cut -d':' -f 1 | grep -q #{group}"
  notifies :request_reboot, "reboot[nginx_user]", :immediately
end

reboot "nginx_user" do
  reason "Potentially have changed uid/gid of system users"
  action :nothing
end

group group do
  system true
  gid gid
end

user user do
  system true
  shell "/bin/nologin"
  uid uid
  gid gid
end

pod_extra_conf = conf["pod_extra_config"].to_a

podman_pod "web" do
  config(
    Pod: pod_extra_conf + %w{
      PublishPort=[::]:80:80/tcp
      PublishPort=80:80/tcp
      PublishPort=[::]:443:443/tcp
      PublishPort=443:443/tcp
    }
  )
end
container_paths = node["podman"]["nginx"]["container"]

directory www
%W{
  #{www}/etc
  #{www}/etc/conf.d
  #{www}/etc/default.d
  #{www}/vhosts
  #{www}/vhosts/default
}.each do |dir|
  directory dir do
    mode "0755"
    owner "root"
    group "root"
  end
end

%W{#{www}/logs #{www}/cache}.each do |dir|
  directory dir do
    owner user
    group group
    mode "0755"
  end
end

logrotate_app "nginx" do
  path "#{www}/logs/*.log"
  frequency "daily"
  rotate 15
  create "644 #{user} #{group}"
end

template "#{www}/etc/conf.d/000-default.conf" do
  source "default_vhost.erb"
  variables(
    paths: container_paths
  )
end

remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv4.txt" do
  source "https://www.cloudflare.com/ips-v4"
  notifies :create, "template[#{www}/etc/cloudflare.conf]"
end

remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv6.txt" do
  source "https://www.cloudflare.com/ips-v6"
  notifies :create, "template[#{www}/etc/cloudflare.conf]"
end

template "#{www}/etc/cloudflare.conf" do
  source "cloudflare_realip.erb"
  variables(
    lazy do
      {
        ipv4: ::IO.read("#{Chef::Config[:file_cache_path]}/cloudflare-ipv4.txt").split,
        ipv6: ::IO.read("#{Chef::Config[:file_cache_path]}/cloudflare-ipv6.txt").split,
      }
    end
  )
  action :nothing
  notifies :reload, "service[nginx]", :delayed
end

podman_image "nginx" do
  config(
    Image: ["Image=docker.io/nginx:stable"],
  )
end

certificates_d = "#{node["podman"]["nginx"]["acme"]["certs_dir"]}/certificates/"

podman_container "nginx" do
  config(
    Container: %W{
      Pod=web.pod
      Image=nginx.image
      Volume=#{www}/etc/conf.d:#{container_paths["etc"]}/conf.d:ro
      Volume=#{www}/etc/default.d:#{container_paths["etc"]}/default.d:ro
      Volume=#{www}/etc/cloudflare.conf:#{container_paths["etc"]}/cloudflare.conf:ro
      Volume=#{www}/vhosts:#{container_paths["www"]}:ro
      Volume=#{www}/logs:#{container_paths["logs"]}
      Volume=#{www}/cache:#{container_paths["cache"]}
      Volume=#{certificates_d}:#{container_paths["ssl"]}:ro
      Volume=/sys:/sys:ro
    },
    Service: [
      "Restart=always",
      "ExecReload=podman exec nginx nginx -t",
      "ExecReload=podman exec nginx nginx -s reload",
    ],
    Unit: [
      "Description=Start NGINX web server",
      "After=network.target",
      "Wants=network-online.target",
    ],
    Install: [
      "WantedBy=multi-user.target dafault.target"
    ]
  )
end

service "nginx" do
  action :start
end
