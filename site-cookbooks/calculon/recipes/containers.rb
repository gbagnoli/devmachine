if platform?("rocky")
  # in a spiral of sadness, rocky's podman
  # doesn't have btrfs enabled
  conf = node["calculon"]["rocky"]["podman"]
  arch = node["kernel"]["machine"]
  rpm = "#{conf[:package]}-#{conf[:version]}.#{arch}.rpm"
  remote = "#{conf[:url]}/#{rpm}"
  local = "#{Chef::Config[:file_cache_path]}/#{rpm}"

  remote_file local do
    source remote
    action :create
  end

  execute "install podman" do
    command "dnf install --assumeyes #{local}"
    not_if "dnf list installed podman | grep -q #{conf[:version]}"
  end
end

package "virt" do
  package_name %w(crun podman)
end

execute "reset podman" do
  command "podman system reset -f"
  action :nothing
end

path = node["calculon"]["containers"]["storage"]["volume"]

execute "create subvolume at #{path}" do
  command "btrfs subvolume create #{path}"
  not_if "btrfs subvolume show #{path} &>/dev/null"
end

template "/etc/containers/storage.conf" do
  variables node["calculon"]["containers"]["storage"]
  source "podman_storage.conf.erb"
  owner "root"
  group "root"
  mode "0755"
  notifies :run, "execute[reset podman]", :before
end
