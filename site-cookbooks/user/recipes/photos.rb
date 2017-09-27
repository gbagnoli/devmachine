%w[exiftool python-wxgtk3.0 python-imaging python-unidecode
   libfreeimage3 libfontconfig1:i386 libxt6:i386 libxrender1:i386
   libxext6:i386 libgl1-mesa-glx:i386 libgl1-mesa-dri:i386 libcurl3:i386
   libgssapi-krb5-2:i386 librtmp1:i386 libsm6:i386 libice6:i386
   libuuid1:i386 gpsbabel gpsbabel-gui].each do |pkg|
     package pkg
   end

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

git "#{home}/.local/src/gpicsync" do
  repository 'https://github.com/metadirective/GPicSync.git'
  action :sync
  user user
end

file "#{home}/.local/bin/gpicsync" do
  content <<EOC
#!/bin/bash
cd #{home}/.local/src/gpicsync/src/
/usr/bin/python2.7 gpicsync.py "$@"
EOC
  owner user
  group node['user']['group']
  mode 0750
end

file "#{home}/.local/bin/gpicsync-GUI" do
  content <<EOC
#!/bin/bash
cd #{home}/.local/src/gpicsync/src/
/usr/bin/python2.7 gpicsync-GUI.py "$@"
EOC
  owner user
  group node['user']['group']
  mode 0750
end

git "#{home}/workspace/photo_process" do
  repository 'https://gist.github.com/28565417cfb732cbd2df784819a7fcb0.git'
  action :sync
end
