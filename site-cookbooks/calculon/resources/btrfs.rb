resource_name :calculon_btrfs_volume
provides :calculon_btrfs_volume
unified_mode true

property :owner, [String, NilClass]
property :group, [String, NilClass]
property :mode, String, default: "0755"
property :setfacl, [true, false], default: false
default_action :create

action :create do
  path = new_resource.name

  execute "create subvolume at #{path}" do
    command "btrfs subvolume create #{path}"
    not_if "btrfs subvolume show #{path} &>/dev/null"
  end

  directory path do
    group new_resource.group || "root"
    owner new_resource.owner || "root"
    mode new_resource.mode
  end

  if new_resource.setfacl
    execute "setfacl_#{path}" do
      command "setfacl -R -d -m g::rwx -m o::rx #{path}"
      user "root"
      not_if "getfacl #{path} 2>/dev/null | grep 'default:' -q"
    end
  end
end
