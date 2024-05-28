install_type = node["syncthing"]["install_type"]

case install_type
when "users"
  node.override["syncthing"]["repo_action"] = "add"
  node.override["syncthing"]["package_action"] = "upgrade"
  include_recipe "syncthing::users"
when "podman"
  node.override["syncthing"]["repo_action"] = "remove"
  node.override["syncthing"]["package_action"] = "purge"
  node.override["syncthing"]["users"] = {}
  include_recipe "syncthing::users"
  include_recipe "syncthing::podman"
when nil
  Chef::Log.fatal("node['syncthing']['install_type'] not set")
  raise
else
  Chef::Log.fatal("invalid value '#{install_type}' for node['syncthing']['install_type']")
  raise
end
