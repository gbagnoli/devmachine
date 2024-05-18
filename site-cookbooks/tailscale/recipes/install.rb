install_type = node["tailscale"]["install_type"]

case install_type
when "distro"
  node.override["tailscale"]["package_action"] = "install"
  include_recipe "tailscale::package"
when "podman"
  node.override["tailscale"]["package_action"] = "uninstall"
  include_recipe "tailscale::package"
  include_recipe "tailscale::podman"
when nil
  Chef::Log.fatal("node['tailscale']['install_type'] not set")
  raise
else
  Chef::Log.fatal("invalid value '#{install_type}' for node['tailscale']['install_type']")
  raise
end
