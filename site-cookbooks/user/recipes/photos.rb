# frozen_string_literal: true

%w[exiftool python-wxgtk3.0 python-pil python-unidecode
   libfreeimage3 libfontconfig1:i386 libxt6:i386 libxrender1:i386
   libxext6:i386 libgl1-mesa-glx:i386 libgl1-mesa-dri:i386 libcurl3:i386
   libgssapi-krb5-2:i386 librtmp1:i386 libsm6:i386 libice6:i386
   libuuid1:i386 fonts-liberation lsb-core libglu1-mesa
   gpsbabel gpsbabel-gui libqtcore4].each do |pkg|
  package pkg
end

package "python-imaging" if node["lsb"]["codename"] == "xenial"

user = node["user"]["login"]
home = "/home/#{user}"

package "python-tz"

git "#{home}/.local/src/gpicsync" do
  repository "https://github.com/metadirective/GPicSync.git"
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
  group node["user"]["group"]
  mode 0o750
end

file "#{home}/.local/bin/gpicsync-GUI" do
  content <<~EOC
                            #!/bin/bash
                            cd #{home}/.local/src/gpicsync/src/
    /usr/bin/python2.7 gpicsync-GUI.py "$@"
          EOC
  owner user
  group node["user"]["group"]
  mode 0o750
end

git "#{home}/workspace/photo_process" do
  repository "git@github.com:gbagnoli/photo_process.git"
  revision "master"
  checkout_branch "master"
  user user
  group node["user"]["group"]
  action :sync
end

version = node["lsb"]["release"]

apt_repository "gpxsee" do
  uri "http://download.opensuse.org/repositories/home:/tumic:/GPXSee/xUbuntu_#{version}/"
  distribution "/"
  components [""]
  key "https://download.opensuse.org/repositories/home:tumic:GPXSee/xUbuntu_#{version}/Release.key"
end

package "gpxsee"

directory "#{home}/.local/src/gps_track_pod" do
  action :delete
  recursive true
end

file "/etc/udev/rules.d/49-gpspod.rules" do
  action :delete
end
