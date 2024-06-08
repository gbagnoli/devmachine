node.override["syncthing"]["install_type"] = "podman"

group "syncthing" do
  comment "syncthing group"
  system true
  gid node["boxy"]["syncthing"]["gid"]
end

user "syncthing" do
  comment "syncthing user"
  system true
  shell "/bin/nologin"
  uid node["boxy"]["syncthing"]["uid"]
  gid node["boxy"]["syncthing"]["gid"]
end

syncd = "#{node["boxy"]["storage"]["path"]}/sync"

directory syncd do
  owner "syncthing"
  group "syncthing"
  mode "2750"
end

execute "setfacl_#{syncd}" do
  command "setfacl -R -d -m g::rwx -m o::rx #{syncd}"
  user "root"
  not_if "getfacl #{syncd} 2>/dev/null | grep 'default:' -q"
end


node.override["syncthing"]["podman"] = {
  "directory" => syncd,
  "uid" => node["boxy"]["syncthing"]["uid"],
  "gid" => node["boxy"]["syncthing"]["gid"],
  "ipv6" => {
    "gui" => "::",
    "service" => "::",
  },
  "ipv4" => {
    "gui" => "",
    "service" => "",
  },
}

include_recipe "syncthing::default"
