package "nginx" do
  action :purge
end

podman_pod "web" do
  config(
    Pod: %w{
      Network=calculon.network
      PublishPort=[::]:80:80/tcp
      PublishPort=80:80/tcp
      PublishPort=[::]:443:443/tcp
      PublishPort=443:443/tcp
    }
  )
end

www = node["calculon"]["storage"]["paths"]["www"]

calculon_btrfs_volume www

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

user = node["calculon"]["nginx"]["user"]
group = node["calculon"]["nginx"]["group"]
uid = node["calculon"]["nginx"]["uid"]
gid = node["calculon"]["nginx"]["gid"]

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
  source "nginx_default_vhost.erb"
  variables(
    paths: node["calculon"]["nginx"]["container"]
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

certificates = "#{node["calculon"]["acme"]["certs_dir"]}/certificates/"
cpaths = node["calculon"]["nginx"]["container"]

podman_container "nginx" do
  config(
    Container: %W{
      Pod=web.pod
      Image=nginx.image
      Volume=#{www}/etc/conf.d:#{cpaths["etc"]}/conf.d:ro
      Volume=#{www}/etc/default.d:#{cpaths["etc"]}/default.d:ro
      Volume=#{www}/etc/cloudflare.conf:#{cpaths["etc"]}/cloudflare.conf:ro
      Volume=#{www}/vhosts:#{cpaths["www"]}:ro
      Volume=#{www}/logs:#{cpaths["logs"]}
      Volume=#{www}/cache:#{cpaths["cache"]}
      Volume=#{certificates}:#{cpaths["ssl"]}:ro
      Volume=/sys:/sys:ro
    },
    Service: [
      "Restart=always",
      "ExecReload=podman exec nginx nginx -t",
      "ExecReload=podman exec nginx nginx -s reload",
    ],
    Unit: [
      "Description=Start NGINX web server",
      "After=network-online.target",
      "Wants=network-online.target",
    ],
    Install: %w{
      WantedBy=multi-user.target
    }
  )
end

calculon_firewalld_port "nginx" do
  port %w{80/tcp 443/tcp}
end

service "nginx" do
  action :start
end
