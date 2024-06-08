conf = node["podman"]["nginx"]["oauth2_proxy"]
conf["secrets"].each do |secret, value|
  if value.nil?
    Chef::Log.info("oauth2_proxy: missing secrets for #{secret}")
    raise
  end
end

include_recipe "podman_nginx::default"

podman_image "oauth2_proxy" do
  config(
    Image: ["Image=quay.io/oauth2-proxy/oauth2-proxy:latest"]
  )
end

nogroup = value_for_platform_family(
  "rhel" => "nobody",
  "debian" => "nogroup",
  "default" => "nobody"
)

user "oauth2proxy" do
  system true
  shell "/bin/nologin"
  group nogroup
  uid "65532"
end

directory "/etc/oauth2_proxy" do
  owner "oauth2proxy"
  group "root"
  mode "0700"
end
