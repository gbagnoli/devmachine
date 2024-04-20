include_recipe "podman::install"

execute "podman_system_reset" do
  command "podman system reset -f"
  action :nothing
end
