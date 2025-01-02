%w[
/lib/systemd/system-shutdown/argononed-poweroff.py
/usr/bin/argononed.py
/usr/bin/argon_temp_monitor
/etc/systemd/system/argononed.service
].each do |f|
  file f do
    action :delete
  end
end

remote_file "#{Chef::Config[:file_cache_path]}/argon1.sh" do
  source "https://download.argon40.com/argon1.sh"
  mode "0755"
  notifies :run, "execute[install_argonone_scripts]", :immediately
end

execute "install_argonone_scripts" do
  action :nothing
  command "#{Chef::Config[:file_cache_path]}/argon1.sh"
end
