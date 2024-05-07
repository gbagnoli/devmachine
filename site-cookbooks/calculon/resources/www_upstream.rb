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
property :category, String, default: "Tools"
property :upstream_address, String, default: "[::1]"
property :upstream_port, [String, Integer], required: true
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
property :extra_properties, [Hash, NilClass]
property :upgrade, [String, NilClass, true, false], default: false
property :matcher, [String, NilClass]
default_action :add

action :add do
  node.override["calculon"]["www"]["upstreams"][new_resource.path] = {
    "upstream" => upstream_url,
    "title" => new_resource.title,
    "extra_properties" => new_resource.extra_properties.to_h,
    "upgrade" => upgrade,
    "matcher" => new_resource.matcher,
    "category" => new_resource.category,
  }
end

action :remove do
  # not adding it equals removing it
end

action_class do
  def upstream_url
    "#{new_resource.upstream_protocol}://#{new_resource.upstream_address}:#{new_resource.upstream_port}"
  end

  def upgrade
    up = new_resource.upgrade
    if up.is_a? String
      up
    elsif up
      '"upgrade"'
    else
      nil
    end
  end
end
