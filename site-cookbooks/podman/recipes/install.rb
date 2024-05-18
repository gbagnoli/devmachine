if platform?("rocky")
  include_recipe "podman::rocky"
elsif platform?("ubuntu")
  include_recipe "podman::ubuntu"
else
  Chef::Log.fatal("platform not supported by podman cookbook -  #{node["platform"]}")
  raise
end
