package "nginx"

service "nginx" do
  action :start
end

calculon_firewalld_port "nginx" do
  port %w{80/tcp 443/tcp}
end
# install lego for letsencrypt

ruby_block "get lego latest version" do
  block do
    uri = URI("https://api.github.com/repos/go-acme/lego/releases/latest")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)
    asset = parsed["assets"].select {|x| x["name"].include?("linux_amd64")}.first
    node.run_state["lego_download_url"] = asset["browser_download_url"]
    node.run_state["lego_version"] = parsed["tag_name"][1..]
  end
end

remote_file "#{Chef::Config[:file_cache_path]}/lego.latest.tar.gz" do
  source(lazy { node.run_state["lego_download_url"] })
  notifies :run, "execute[install_lego]", :immediately
end

execute "install_lego" do
  action :nothing
  command "tar -xpzf #{Chef::Config[:file_cache_path]}/lego.latest.tar.gz -C /usr/local/bin lego"
end

directory node["calculon"]["acme"]["certs_dir"] do
  owner "root"
  group node["calculon"]["nginx"]["group"]
  mode "0755"
end

certificates_d = "#{node["calculon"]["acme"]["certs_dir"]}/certificates"
directory certificates_d  do
  owner "root"
  group node["calculon"]["nginx"]["group"]
  mode "2750"
end

execute "setfacl_#{certificates_d}" do
  command "setfacl -R -d -m g::r -m o::- #{certificates_d}"
  user "root"
  not_if "getfacl #{certificates_d} 2>/dev/null | grep 'default:' -q"
end

file "/etc/nginx/default.d/lego.conf" do
  content <<~EOH
  location /.well-known/acme-challenge/ {
      proxy_pass http://127.0.0.1:#{node["calculon"]["acme"]["lego"]["port"]};
      proxy_set_header Host $host;
  }
  EOH
  notifies :reload, "service[nginx]", :immediately
end

raise 'Password not set for ACME - node["calculon"]["acme"]["lego"]["email"]' if node["calculon"]["acme"]["lego"]["email"].nil?

template "/usr/local/bin/lego_periodic_renew" do
  source "lego_periodic_renew.sh.erb"
  mode "0755"
  user "root"
  group "root"
  variables(
    lego_path: node["calculon"]["acme"]["certs_dir"],
    lego_port: node["calculon"]["acme"]["lego"]["port"],
    email: node["calculon"]["acme"]["lego"]["email"],
    lego: "/usr/local/bin/lego",
    key_type: node["calculon"]["acme"]["key_type"],
    renew_days: node["calculon"]["acme"]["renew_days"]
  )
end

template "/usr/local/bin/lego_request" do
  source "lego_request.sh.erb"
  mode "0755"
  user "root"
  group "root"
  variables(
    lego_path: node["calculon"]["acme"]["certs_dir"],
    lego_port: node["calculon"]["acme"]["lego"]["port"],
    email: node["calculon"]["acme"]["lego"]["email"],
    lego: "/usr/local/bin/lego",
    key_type: node["calculon"]["acme"]["key_type"],
  )
end

# TODO install systemd timer
# https://go-acme.github.io/lego/usage/cli/renew-a-certificate/
systemd_unit "lego_renew_certificates.service" do
	content <<~EOH
    [Unit]
    Description=Renew certificates from letsencrypt

    [Service]
    type=oneshot
    ExecStart=/usr/local/bin/lego_periodic_renew
    User=root
    Group=systemd-journal
	EOH
  action %i(create enable)
end

systemd_unit "lego_renew_certificates.timer" do
  content <<~EOH
    [Unit]
    Description=Renew certificates

    [Timer]
    Unit=lego_renew_certificates.service
    Persistent=true

    # instead, use a randomly chosen time:
    OnCalendar=*-*-* 6:13
    # add extra delay, here up to 1 hour:
    RandomizedDelaySec=1h

    [Install]
    WantedBy=timers.target
  EOH
  action %i(create enable start)
end

directory "/var/www" do
  owner "root"
  group "root"
  mode "0755"
end

directory "/var/log/nginx/vhosts" do
  owner node["calculon"]["nginx"]["user"]
  group node["calculon"]["nginx"]["group"]
  mode "0755"
end

remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv4.txt" do
  source "https://www.cloudflare.com/ips-v4"
  notifies :create, "template[/etc/nginx/cloudflare.conf]"
end

remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv6.txt" do
  source "https://www.cloudflare.com/ips-v6"
  notifies :create, "template[/etc/nginx/cloudflare.conf]"
end

template "/etc/nginx/cloudflare.conf" do
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
