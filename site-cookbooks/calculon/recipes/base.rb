include_recipe "calculon::repos"

if platform?("rocky")
  # sad, but rocky doesn't have btrfs-progs in repos
  conf = node["calculon"]["rocky"]["btfrs_progs"]
  arch = node["kernel"]["machine"]
  v, r = conf[:version].split("-")
  url = "#{conf[:url]}/#{v}/#{r}/#{arch}"
  packages = []

  conf[:packages].each do |pkg|
    rpm = "#{pkg}-#{conf[:version]}.#{arch}.rpm"
    remote = "#{url}/#{rpm}"
    local = "#{Chef::Config[:file_cache_path]}/#{rpm}"
    packages << local

    remote_file local do
      source remote
      action :create
    end
  end

  execute "install btrfs-progs" do
    command "dnf install --assumeyes #{packages.join(" ")}"
    not_if "dnf list installed btrfs-progs | grep -q #{conf[:version]}"
  end

  # also install kernel-ml as there will be no btrfs otherwise
  package "kernel-ml" do
    notifies :reboot_now, "reboot[install_kernel_ml]", :immediately
  end

  reboot "install_kernel_ml" do
    reason "New kernel has been installed"
    action :nothing
  end
end

package "base" do
  package_name %w(curl htop iotop iperf btrfs-progs acl)
end

data_user = node["calculon"]["data"]["username"]
data_group = node["calculon"]["data"]["group"]

group data_group do
  gid node["calculon"]["data"]["gid"]
  members users
  append true
end

user data_user do
  uid node["calculon"]["data"]["uid"]
  gid node["calculon"]["data"]["gid"]
  system true
  shell "/bin/false"
end

paths = node["calculon"]["storage"]["paths"]

directory paths["root"]

if node["calculon"]["storage"]["manage"]
  mount paths["root"] do
    device node["calculon"]["storage"]["dev"]
    fstype "btrfs"
    options %w{rw noatime compress=zstd:3,space_cache=v2,autodefrag}
    action %i(mount enable)
  end
end

%w{sync media downloads library}.each do |vol|
  path = paths[vol]

  execute "create subvolume at #{path}" do
    command "btrfs subvolume create #{path}"
    not_if "btrfs subvolume show #{path} &>/dev/null"
  end

  directory path do
    group data_group
    owner data_user
    mode "2775"
  end

  execute "setfacl_#{path}" do
    command "setfacl -R -d -m g::rwx -m o::rx #{path}"
    user "root"
    not_if "getfacl #{path} 2>/dev/null | grep 'default:' -q"
  end
end

node["calculon"]["storage"]["library_dirs"].each do |dir|
  %w{downloads library}.each do |parent|
    path = "#{paths[parent]}/#{dir}"
    directory path do
      group data_group
      owner data_user
      mode "2775"
    end
  end
end

package "et"
package "tmux"

service "et" do
  action %i{enable start}
end

execute "open et port" do
  command "firewall-cmd --zone=public --add-port=2022/tcp"
  not_if "firewall-cmd --zone=public --list-ports | grep -q 2022/tcp"
  notifies :run, "execute[persist_firewalld]"
end
