package "base" do
  package_name %w(curl htop iotop iperf btrfs-progs acl)
end

group node["calculon"]["data"]["group"] do
  gid node["calculon"]["data"]["gid"]
  members users
  append true
end

user node["calculon"]["data"]["username"] do
  uid node["calculon"]["data"]["uid"]
  gid node["calculon"]["data"]["gid"]
  system true
  shell "/bin/false"
end

%w{root sync media downloads library}.each do |vol|
  path = node["calculon"]["paths"][vol]

  execute "create subvolume at #{path}" do
    command "btrfs subvolume create #{path}"
    not_if "btrfs subvolume show #{path} &>/dev/null"
  end

  directory path do
    group node["calculon"]["data"]["group"]
    owner node["calculon"]["data"]["username"]
    mode "2775"
  end

  execute "setfacl_#{path}" do
    command "setfacl -R -d -m g::rwx -m o::rx #{path}"
    user "root"
    not_if "getfacl #{path} 2>/dev/null | grep 'default:' -q"
  end
end

node["calculon"]["paths"]["library_dirs"].each do |dir|
  %w{downloads library}.each do |parent|
    path = "#{node["calculon"]["paths"][parent]}/#{dir}"
    directory path do
      group node["calculon"]["data"]["group"]
      owner node["calculon"]["data"]["username"]
      mode "2775"
    end
  end
end
