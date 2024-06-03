package "btrfs-progs"

return if node["boxy"]["skip_mounts"]

root = node["boxy"]["storage"]["path"]
dev = node["boxy"]["storage"]["dev"]
directory root

mount root do
  device dev
  fstype "btrfs"
  action %i(mount enable)
end

root = "#{node["boxy"]["storage"]["path"]}/containers"
user = node["user"]
system_containers_path = "#{root}/system"
user_containers_path = "#{root}/#{user["login"]}"

directory root do
  mode '755'
end

[{path: system_containers_path, owner: "root", group: "root",
runroot: "/run/containers/storage", config:"/etc/containers/storage.conf"},
 {path: user_containers_path, owner: user["login"], group: user["group"], runroot: nil,
config: "/home/#{user["login"]}/.config/containers/storage.conf"}].each do |info|
  execute "create containers subvolume #{info[:path]}" do
    not_if { ::File.directory?(info[:path]) }
    command "btrfs subvolume create #{info[:path]}"
  end

  directory info[:path] do
    owner info[:owner]
    group info[:group]
    mode '775'
  end

  directory File.dirname(info[:config]) do
    owner info[:owner]
    group info[:group]
    mode '775'
    recursive true
  end

  template info[:config] do
    source "storage.conf.erb"
    variables(
      runroot: info[:runroot],
      graphroot: info[:path]
    )
  end
end
