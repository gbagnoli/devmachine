include_recipe "nginx"

package "ssl-cert"

directory node["bender"]["certificates"]["directory"] do
  mode "0700"
end

directory "/var/www"

unless Chef::Config[:why_run]
  remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv4.txt" do
    source "https://www.cloudflare.com/ips-v4"
    notifies :create, "template[/etc/nginx/cloudflare.conf]"
  end

  remote_file "#{Chef::Config[:file_cache_path]}/cloudflare-ipv6.txt" do
    source "https://www.cloudflare.com/ips-v6"
    notifies :create, "template[/etc/nginx/cloudflare.conf]"
  end

  template "/etc/nginx/cloudflare.conf" do
    source "nginx/realip.erb"
    variables(
      lazy do
        {
          ipv4: ::IO.read("#{Chef::Config[:file_cache_path]}/cloudflare-ipv4.txt").split,
          ipv6: ::IO.read("#{Chef::Config[:file_cache_path]}/cloudflare-ipv6.txt").split,
        }
      end
    )
    action :nothing
    notifies :reload, "service[nginx]"
  end
end

# we need to restart shorewall in case this is the first run and we need to
# open the ports, or acme validation will fail.
# this sucks.
# FIXME: commented this out as restart is disruptive for connections in container
# need to trigger this when installing nginx, or simply let first run fail
# %w[shorewall shorewall6].each do |fw|
#   service "restart_#{fw}" do
#     service_name fw
#     action :restart
#   end
# end

node["bender"]["vhosts"].each do |vhost, conf|
  bender_vhost vhost do
    server_name conf["server_name"]
    port conf["port"]
    upstream_url conf["upstream_url"]
    container conf["container"]
    ssl conf["ssl"]
    letsencrypt conf["letsencrypt"]
    letsencrypt_common_name conf["letsencrypt_common_name"]
    letsencrypt_contact conf["letsencrypt_contact"]
    letsencrypt_alt_names conf["letsencrypt_alt_names"]
    ssl_cert_path conf["ssl_cert_path"]
    ssl_key_path conf["ssl_key_path"]
    cloudflare conf["cloudflare"]
  end
end
