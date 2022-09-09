package "mongodb-server"
package "openjdk-8-jre-headless"
package "jsvc"
package "binutils"
package "coreutils"
package "adduser"
package "libcap2"

packages = {
  "unifi" => "http://dl.ubnt.com/unifi/5.6.30/unifi_sysvinit_all.deb",
}

packages.each do |name, url|
  debfile = "/usr/src/#{name}.deb"
  remote_file debfile do
    action :create
    source url
    notifies :run, "execute[install_#{name}]", :immediately
  end

  execute "install_#{name}" do
    action :nothing
    command "dpkg -i #{debfile}"
  end
end

service "mongodb" do
  action %i(stop disable)
end

service "unifi" do
  action %i(start enable)
end
