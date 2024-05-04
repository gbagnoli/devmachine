resource_name :calculon_www_link
provides :calculon_www_link
unified_mode true
property :url, String
property :category, String, default: "Tools"
default_action :add
action :add do
  node.override["calculon"]["www"]["upstreams"][new_resource.url] = {
    "title" => new_resource.name,
    "category" => new_resource.category,
    "link_only" => true
  }
end

action :remove do
  # not adding it equals removing it
end
