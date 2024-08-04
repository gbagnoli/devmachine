# frozen_string_literal: true

if platform?("ubuntu") && ! %w(focal jammy noble).include?(node["lsb"]["codename"])
  %w(exiftool python-wxgtk3.0 python-pil python-unidecode
   libfreeimage3 libfontconfig1:i386 libxt6:i386 libxrender1:i386
   libxext6:i386 libgl1-mesa-glx:i386 libgl1-mesa-dri:i386 libcurl3:i386
   libgssapi-krb5-2:i386 librtmp1:i386 libsm6:i386 libice6:i386
   libuuid1:i386 fonts-liberation lsb-core libglu1-mesa
   gpsbabel gpsbabel-gui libqtcore4 python-tz rename).each do |pkg|
    package pkg
  end
else
  package "photo_deps" do
    package_name %w(exiftool gpsbabel python3-tz rename)
  end
end

user = node["user"]["login"]
home = "/home/#{user}"

git "#{home}/.local/src/gpicsync" do
  repository "https://github.com/FrancoisSchnell/GPicSync.git"
  action :sync
  user user
end

file "#{home}/.local/bin/gpicsync" do
  content <<~EOC
    #!/bin/bash
    cd #{home}/.local/src/gpicsync/src/
    /usr/bin/python3 gpicsync.py "$@"
          EOC
  owner user
  group node["user"]["group"]
  mode '750'
end

file "#{home}/.local/bin/gpicsync-GUI" do
  content <<~EOC
    #!/bin/bash
    cd #{home}/.local/src/gpicsync/src/
    /usr/bin/python gpicsync-GUI.py "$@"
          EOC
  owner user
  group node["user"]["group"]
  mode '750'
end

cookbook_file "#{home}/.local/src/exiftool_GPS2MapUrl.config" do
  owner user
  group node["user"]["group"]
  mode '644'
  source "exiftool_GPS2MapUrl.config"
end

if node["user"]["install_photo_process"]
  git "#{home}/workspace/photo_process" do
    repository "git@github.com:gbagnoli/photo_process.git"
    revision "master"
    checkout_branch "master"
    enable_checkout false
    user user
    group node["user"]["group"]
    action :sync
  end
end

version = node["lsb"]["release"]

apt_repository "gpxsee" do
  uri "http://download.opensuse.org/repositories/home:/tumic:/GPXSee/xUbuntu_#{version}/"
  distribution "/"
  components [""]
  key "https://download.opensuse.org/repositories/home:tumic:GPXSee/xUbuntu_#{version}/Release.key"
end

package "gpxsee"
