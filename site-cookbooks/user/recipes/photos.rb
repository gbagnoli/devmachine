# frozen_string_literal: true

package "photo_deps" do
  package_name %w(exiftool gpsbabel python3-tz rename lsb-release)
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
