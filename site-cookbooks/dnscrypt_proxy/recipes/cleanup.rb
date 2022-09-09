# cleanup old repo
file "/etc/default/dnscrypt-proxy" do
  action :delete
end

apt_repository "dnscrypt" do
  action :remove
  notifies :purge, "apt_package[dnscrypt-proxy]", :immediately
end

apt_package "dnscrypt-proxy" do
  action :nothing
end

%w(dnscrypt-autoinstall dnscrypt-autoinstall-backup).each do |svc|
  service svc do
    action :nothing
  end
end

directory node["dnscrypt_proxy"]["autoinstall_src_dir"] do
  action :delete
  recursive true
  notifies :stop, "service[dnscrypt-autoinstall]", :immediately
  notifies :stop, "service[dnscrypt-autoinstall-backup]", :immediately
end

%w(autoinstall.service autoinstall-backupi.service autoinstall.conf).each do |f|
  file "/etc/systemd/system/dnscrypt-#{f}" do
    action :delete
  end
end
