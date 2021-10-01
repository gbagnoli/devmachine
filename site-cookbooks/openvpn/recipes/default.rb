versions = node["openvpn"]["known_versions"]
version = node["openvpn"]["version"]

raise "Unkown version #{version}" unless versions.include?(version)

distro = node["lsb"]["codename"]
distro = node["openvpn"]["override_distribution"] if node["openvpn"]["override_distribution"]

if %w[wheezy jessie precise trusty xenial].include?(distro)
  apt_repository "openvpn" do
    uri "http://build.openvpn.net/debian/openvpn/#{version}"
    distribution distro
    components ["main"]
    key "https://swupdate.openvpn.net/repos/repo-public.gpg"
  end
else
  Chef::Log.info("Not adding PPA for openvpn as #{distro} is not supported")
end

package "openvpn"

file "/etc/systemd/system/openvpn@.service" do
  action :delete
end

directory "/etc/systemd/system/openvpn@.service.d" do
  mode 0o750
  owner "root"
  group "root"
end

file "/etc/systemd/system/openvpn@.service.d/override.conf" do
  mode 0o644
  owner "root"
  group "root"
  content <<~EOH
    [Service]
    LimitNPROC=infinity
  EOH
  notifies :run, "execute[reload-systemd-openvpn]", :immediately
end

execute "reload-systemd-openvpn" do
  action :nothing
  command "systemctl daemon-reload"
end
