node["calculon"]["oauth2_proxy"]["secrets"].each do |secret, value|
  if value.nil?
    raise "oauth2_proxy: secrets missing node[\"calculon\"][\"oauth2_proxy\"][\"secrets\"][\"#{secret}\"]"
  end
end

include_recipe "calculon::nginx"
chef_gem "toml"

podman_image "oauth2_proxy" do
  config(
    Image: ["Image=quay.io/oauth2-proxy/oauth2-proxy:latest"]
  )
end

user "oauth2proxy" do
  system true
  shell "/bin/nologin"
  group "nobody"
  uid 65532
end

directory "/etc/oauth2_proxy" do
  owner "oauth2proxy"
  group "root"
  mode "0700"
end
