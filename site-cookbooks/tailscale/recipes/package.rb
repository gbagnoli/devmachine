unless platform_family?("debian")
  if node["tailscale"]["package_action"] == "install"
    Chef::Log.fatal("tailscale: install_type 'distro' only supported for debian")
    raise
  end
end

case node["tailscale"]["package_action"]
when "install"
  pkg_action = :upgrade
  repo_action = :add
  service_action = %i[start enable]
when "uninstall"
  pkg_action = :purge
  repo_action = :remove
  service_action = %i[disable stop]
else
  Chef::Log.fatal("Invalid action #{node["tailscale"]["package_action"]}")
  raise
end

apt_repository 'tailscale' do
  uri 'https://pkgs.tailscale.com/stable/ubuntu'
  key 'https://pkgs.tailscale.com/stable/ubuntu/jammy.noarmor.gpg'
  components ['main']
  action repo_action
end

package 'tailscale' do
  action pkg_action
end

service 'tailscaled' do
  action service_action
end
