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
      not_if "dnf list installed btrfs-progs | grep -q #{conf[:version]}"
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

%w{sync media}.each do |vol|
  calculon_btrfs_volume paths[vol] do
    group data_group
    owner data_user
    mode "2775"
    setfacl true
  end
end

node["calculon"]["storage"]["library_dirs"].each_key do |dir|
  path = "#{paths["media"]}/#{dir}"
  ["", "/downloads", "/library"].each do |child|
    directory "#{path}#{child}" do
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

calculon_firewalld_port "2022/tcp"

include_recipe "podman"

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
  notifies :run, "execute[podman_system_reset]", :before
end

podman_network "calculon" do
  config(
    Network: %W{
      Driver=bridge
      IPv6=True
      Subnet=#{node["calculon"]["network"]["containers"]["ipv4"]["network"]}
      Subnet=#{node["calculon"]["network"]["containers"]["ipv6"]["network"]}
      Gateway=#{node["calculon"]["network"]["containers"]["ipv4"]["addr"]}
      Gateway=#{node["calculon"]["network"]["containers"]["ipv6"]["addr"]}
    }
  )
end

include_recipe "btrbk"

volumes = node["calculon"]["storage"]["snapshots_volumes"]
snapd = "#{node["calculon"]["storage"]["paths"]["root"]}/snapshots/"
directory snapd
volumes.each do |vol|
  directory "#{snapd}/#{vol}"
end

directory "/etc/btrbk" do
  mode "0755"
end

template "/etc/btrbk/btrbk.conf" do
  mode "0644"
  source "btrbk.conf.erb"
  variables(
    vol: node["calculon"]["storage"]["paths"]["root"],
    snapshotd: "snapshots",
    subvolumes: node["calculon"]["storage"]["snapshots_volumes"]
  )
end

# disable builtin daily timer
systemd_unit 'btrbk.timer' do
  action :disable
end

systemd_unit 'btrbk_hourly.timer' do
  content <<~EOH
    [Unit]
    Description=btrbk hourly backup

    [Timer]
    OnCalendar=hourly
    Persistent=true
    Unit=btrbk.service

    [Install]
    WantedBy=timers.target
  EOH
  action %i{create enable start}
end
