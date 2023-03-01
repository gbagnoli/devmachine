# frozen_string_literal: true

if platform?("ubuntu") && ! %w(focal jammy).include?(node["lsb"]["codename"])
  %w(exiftool python-wxgtk3.0 python-pil python-unidecode
   libfreeimage3 libfontconfig1:i386 libxt6:i386 libxrender1:i386
   libxext6:i386 libgl1-mesa-glx:i386 libgl1-mesa-dri:i386 libcurl3:i386
   libgssapi-krb5-2:i386 librtmp1:i386 libsm6:i386 libice6:i386
   libuuid1:i386 fonts-liberation lsb-core libglu1-mesa
   gpsbabel gpsbabel-gui libqtcore4 python-tz rename).each do |pkg|
    package pkg
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


ruby_block "get gphotos-uploader-cli url" do
  block do
    Chef::Resource::RubyBlock.send(:include, Chef::Mixin::ShellOut) # rubocop:disable Lint/SendWithMixinArgument
    release = "https://api.github.com/repos/gphotosuploader/gphotos-uploader-cli/releases/latest"
    download_url = ".assets[].browser_download_url"
    command = "curl -sL #{release} | jq -r '#{download_url}' | grep linux_amd64"
    out = shell_out(command)
    node.run_state["gphotos_uploader_cli_url"] = out.stdout
  end
  action :run
end

remote_file "/usr/src/gphotos-uploader-cli.tar.gz" do
  source lazy { node.run_state["gphotos_uploader_cli_url"].chomp } # rubocop:disable Lint/AmbiguousBlockAssociation
  mode '644'
  notifies :run, "execute[unpack gphotos uploader cli]", :immediately
end

execute "unpack gphotos uploader cli" do
  command "tar -C /usr/local/bin/ -xzf /usr/src/gphotos-uploader-cli.tar.gz gphotos-uploader-cli"
  action :nothing
end

file "/usr/local/bin/gphotos-uploader-cli" do
  mode '755'
end

user = node["user"]["login"]
group = node["user"]["group"]
home = "/home/#{user}"
gpu_conf_d =  "#{home}/.gphotos-uploader-cli"
gpu_config = "#{gpu_conf_d}/config.hjson"

if node["gphotos_uploader_cli"].nil?
  Chef::Log.error("Skipping gphotos-uploader-cli config as there are no secrets")
  return
end

props = [node["gphotos_uploader_cli"]["ClientID"],
         node["gphotos_uploader_cli"]["ClientSecret"],
         node["gphotos_uploader_cli"]["Account"]]

if props.map(&:nil?).any?
  Chef::Log.error("Skipping gphotos-uploader-cli config as there are no secrets")
  return
end

directory gpu_conf_d do
    action :create
    owner user
    group group
    mode "0700"
end

template gpu_config do
  source "gphotos-uploader-cli_config.hjson.erb"
  mode "0600"
  owner user
  group group
  variables(
    conf: node["gphotos_uploader_cli"]
  )
end
