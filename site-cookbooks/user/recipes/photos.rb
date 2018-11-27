# frozen_string_literal: true

%w[exiftool python-wxgtk3.0 python-pil python-unidecode
   libfreeimage3 libfontconfig1:i386 libxt6:i386 libxrender1:i386
   libxext6:i386 libgl1-mesa-glx:i386 libgl1-mesa-dri:i386 libcurl3:i386
   libgssapi-krb5-2:i386 librtmp1:i386 libsm6:i386 libice6:i386
   libuuid1:i386 fonts-liberation lsb-core libglu1-mesa
   gpsbabel gpsbabel-gui libqtcore4].each do |pkg|
     package pkg
   end

package 'python-imaging' if node['lsb']['codename'] == 'xenial'

package 'googleearth-package' do
  notifies :run, 'execute[make-googleearth-package]', :immediately
end

execute 'make-googleearth-package' do
  action :nothing
  cwd '/tmp'
  command 'make-googleearth-package'
  user node['user']['login']
  notifies :run, 'execute[install_googleearth]', :immediately
end

execute 'install_googleearth' do
  action :nothing
  command 'sudo dpkg -i googleearth_*.deb'
  cwd '/tmp'
  notifies :run, 'execute[delete_googleearth_deb]', :immediately
end

execute 'delete_googleearth_deb' do
  action :nothing
  command 'rm -f googleearth_*.deb GoogleEarthLinux.bin'
  cwd '/tmp'
end

user = node['user']['login']
home = "/home/#{user}"

package 'python-tz'

git "#{home}/.local/src/gpicsync" do
  repository 'https://github.com/metadirective/GPicSync.git'
  action :sync
  user user
end

file "#{home}/.local/bin/gpicsync" do
  content <<~EOC
    #!/bin/bash
    cd #{home}/.local/src/gpicsync/src/
    /usr/bin/python2.7 gpicsync.py "$@"
  EOC
  owner user
  group node['user']['group']
  mode 0o750
end

file "#{home}/.local/bin/gpicsync-GUI" do
  content <<~EOC
    #!/bin/bash
    cd #{home}/.local/src/gpicsync/src/
    /usr/bin/python2.7 gpicsync-GUI.py "$@"
  EOC
  owner user
  group node['user']['group']
  mode 0o750
end

git "#{home}/workspace/photo_process" do
  repository 'https://gist.github.com/28565417cfb732cbd2df784819a7fcb0.git'
  action :sync
end

version = node['lsb']['release']

apt_repository 'gpxsee' do
  uri "http://download.opensuse.org/repositories/home:/tumic:/GPXSee/xUbuntu_#{version}/"
  distribution '/'
  components ['']
  key "https://download.opensuse.org/repositories/home:tumic:GPXSee/xUbuntu_#{version}/Release.key"
end

%w[libudev-dev libusb-1.0-0-dev gpxsee].each do |pkg|
  package pkg
end

git "#{home}/.local/src/gps_track_pod" do
  repository 'https://github.com/iwanders/gps_track_pod.git'
  action :sync
  user user
end

directory '/etc/udev/rules.d' do
  action :create
  recursive true
  mode 0o755
end

file '/etc/udev/rules.d/49-gpspod.rules' do
  owner 'root'
  group 'root'
  mode 0o644
  content(lazy { ::File.open("#{home}/.local/src/gps_track_pod/49-gpspod.rules").read })
  action :create
end
