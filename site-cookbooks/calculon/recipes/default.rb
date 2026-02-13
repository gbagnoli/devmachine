include_recipe "calculon::repos"
include_recipe "calculon::monitoring"
include_recipe "calculon::base"
include_recipe "calculon::zram"
include_recipe "calculon::nginx"
include_recipe "calculon::tailscale"
include_recipe "calculon::media"
include_recipe "calculon::magiusstaff"
include_recipe "calculon::distrobox"
include_recipe "calculon::joplin"

# create the www vhost with the accumulated hosts
domain = node["calculon"]["www"]["domain"]
unless domain.nil?
  podman_nginx_vhost domain do
    server_name domain
    cloudflare true
    upstream_paths(lazy { node["calculon"]["www"]["upstreams"].to_h })
    oauth2_proxy(
      emails: node["calculon"]["www"]["user_emails"],
      port: 4100
    )
    act_as_upstream 4101
  end

  www = node["calculon"]["storage"]["paths"]["www"]
  template "#{www}/vhosts/#{domain}/index.html" do
    source "www_host_index.erb"
    cookbook "podman_nginx"
    variables(
      upstreams: lazy { node["calculon"]["www"]["upstreams"].to_h },
      domain: domain,
    )
  end
end
