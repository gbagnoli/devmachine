include_recipe "calculon::repos"
include_recipe "calculon::monitoring"
include_recipe "calculon::base"
include_recipe "calculon::nginx"
include_recipe "calculon::lego"
include_recipe "calculon::tailscale"

include_recipe "calculon::containers"
include_recipe "calculon::media"

# create the www vhost with the accumulated hosts
unless node["calculon"]["www"]["domain"].nil?
  calculon_vhost node["calculon"]["www"]["domain"] do
    server_name node["calculon"]["www"]["domain"]
    cloudflare true
    upstream_paths lazy { node["calculon"]["www"]["upstreams"].to_h }
    oauth2_proxy(
      emails: node["calculon"]["oauth2_proxy"]["secrets"]["syncthing_authenticated_emails"],
      port: 4100
    )
    act_as_upstream 4101
  end
end


# TODO render default page for upstreams
