include_recipe "podman::install"

service "podman.socket" do
  action %i[enable start]
end
