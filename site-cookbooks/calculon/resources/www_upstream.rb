resource_name :calculon_www_upstream
provides :calculon_www_upstream
unified_mode true

path_callback = {
  "should start with /" => lambda { |path|
    path.start_with?("/")
  },
}

property :path, String, name_property: true, callbacks: path_callback
property :title, [String, NilClass]
property :upstream_address, String, default: "[::1]"
property :upstream_port, [String, Integer], required: true
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
default_action :add

action :add do
  node.override["calculon"]["www"]["upstreams"][new_resource.path]["upstream"] = upstream_url
  node.override["calculon"]["www"]["upstreams"][new_resource.path]["title"] = new_resource.title
end

action :remove do
  node.override["calculon"]["www"]["upstreams"][new_resource.path] = nil
end

action_class do
  def upstream_url
    "#{new_resource.upstream_protocol}://#{new_resource.upstream_address}:#{new_resource.upstream_port}"
  end
end
