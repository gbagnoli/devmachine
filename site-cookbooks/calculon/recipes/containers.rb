package "virt" do
  package_name %w(crun podman)
end

execute "reset podman" do
  command "podman system reset -f"
  action :nothing
end

template "/etc/containers/storage.conf" do
  variables node["calculon"]["containers"]["storage"]
  source "podman_storage.conf.erb"
  owner "root"
  group "root"
  mode "0755"
  notifies :run, "execute[reset podman]", :before
end
