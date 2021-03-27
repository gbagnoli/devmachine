# frozen_string_literal: true

return if node["ubik"]["skip_packages"]

packages = %w[
  apt-transport-https btrfs-tools compizconfig-settings-manager dkms docker-ce
  dstat exfat-fuse exfat-utils firefox gconf-service gconf2 gdm3
  gir1.2-gnomekeyring-1.0 gnome gnome-shell gnome-terminal google-chrome-stable
  google-talkplugin gstreamer1.0-libav gstreamer1.0-plugins-ugly
  gstreamer1.0-pulseaudio gvfs-bin htop keepassx libappindicator1
  libappindicator1 libcurl4 libgcrypt20 libgnome-keyring0 libgtk2.0-0
  libnotify4 libnss3 libsecret-1-0 libudev1 libvirt-dev libxml2-dev
  libxslt1-dev libxss1 libxtst6 network-manager-openvpn-gnome openvpn powertop
  python python-apt qemu-kvm rsyslog shellcheck tmux
  ttf-mscorefonts-installer ubuntu-gnome-desktop ufraw unity-tweak-tool
  xdg-utils wireguard xsel signal-desktop vlc ffmpeg jq telegram
]

if node["lsb"]["codename"] == "focal"
  oldp = %w[btrfs-tools gir1.2-gnomekeyring-1.0 libgnome-keyring0 ufraw libcurl3]
  packages = packages.map(&:dup).reject { |x| oldp.include?(x) }
  packages << "btrfs-progs"
  packages << "virtualbox"
  packages << "dropbox"
else
  packages << "virtualbox6.1"
end

package "base install" do
  package_name packages
  action :install
  timeout 1800
end

packages = {
  "dropbox" => {
    deb: "https://linux.dropbox.com/packages/ubuntu/dropbox_2015.10.28_amd64.deb",
    only_if_not_installed: true,
  }, "keybase" => {
    deb: "https://prerelease.keybase.io/keybase_amd64.deb",
    only_if_not_installed: true,
  }, "skypeforlinux" => {
    deb: "https://repo.skype.com/latest/skypeforlinux-64.deb",
    only_if_not_installed: true,
  }, "slack-desktop" => {
    deb: "https://downloads.slack-edge.com/linux_releases/slack-desktop-4.4.2-amd64.deb",
    only_if_not_installed: true,
  },
  "steam-launcher" => {
    deb: "https://steamcdn-a.akamaihd.net/client/installer/steam.deb",
    only_if_not_installed: true,
  }, "vagrant" => "https://releases.hashicorp.com/vagrant/2.2.7/vagrant_2.2.7_x86_64.deb",
  "discord" => {
    deps: %w[libc++1 libc++abi1],
    deb: "https://discordapp.com/api/download?platform=linux&format=deb",
    only_if_not_installed: false,
  },
  "zoom" => {
    deb: "https://zoom.us/client/latest/zoom_amd64.deb",
    only_if_not_installed: true,
    deps: %w[libgl1-mesa-glx libegl1-mesa libxcb-xtest0],
  }
}

# accept steam license
debconf_selection "steam/question" do
  value "I AGREE"
  package "steam"
  type "select"
  not_if "dpkg -l | grep -E '^ii\s+steam-launcher'"
end

debconf_selection "steam/license" do
  value ""
  type "note"
  package "steam"
  not_if "dpkg -l | grep -E '^ii\s+steam-launcher'"
end

packages.each do |name, desc|
  if desc.is_a? String
    url = desc
    not_if = false
    deps = nil
  else
    url = desc[:deb]
    not_if = desc[:only_if_not_installed]
    deps = desc[:deps]
  end

  deps&.each do |x|
    package x
  end

  not_if = "dpkg -l | grep -E '^ii\s+#{name}'" if not_if

  debfile = "/usr/src/#{name}.deb"
  remote_file debfile do
    action :create
    source url
    notifies :run, "execute[install_#{name}]", :immediately
    not_if not_if
  end

  execute "install_#{name}" do
    action :nothing
    command "dpkg -i #{debfile}"
  end
end

# dropbox daemon
remote_file "/usr/src/dropbox-daemon.tar.gz" do
  source "https://www.dropbox.com/download?plat=lnx.x86_64"
end

# dropbox control daemon from cli
remote_file "/usr/local/bin/dropbox.py" do
  source "https://www.dropbox.com/download?dl=packages/dropbox.py"
  mode "0755"
end

vagrant_plugins = %w[
  vagrant-libvirt
  vagrant-kvm
  vagrant-host-shell
]

vagrant_plugins.each do |plg|
  execute "vagrant_install_#{plg}" do
    command "vagrant plugin install #{plg}"
    not_if "vagrant plugin list | grep -q '^#{plg} '"
  end
end

directory "/opt/android" do
  recursive true
end

# android platform tools
remote_file "/usr/src/android_platform_tools.zip" do
  source "https://dl.google.com/android/repository/platform-tools-latest-linux.zip"
  notifies :run, "execute[unzip_android_platform_tools]", :immediately
end

execute "unzip_android_platform_tools" do
  action :nothing
  command "unzip -o /usr/src/android_platform_tools.zip -d /opt/android/"
end

include_recipe "yubico::personalization"

# VA-API enabled chromium
apt_repository "chromium-beta" do
  uri "ppa:saiarcot895/chromium-dev"
end

package "chromium-browser"
include_recipe "flatpak"

# tzbuddy
include_recipe 'ark'
ark 'tzbuddy' do
  url(lazy do
    uri = URI("https://api.github.com/repos/gbagnoli/tzbuddy.rs/releases/latest")
    response = Net::HTTP.get(uri)
    asset = JSON.parse(response)["assets"].select {|x| x["name"] == "tzbuddy-linux-amd64.tar.gz"}.first
    asset["browser_download_url"]
  end)
  path '/usr/local/bin'
  creates 'tzbuddy'
  action :cherry_pick
  strip_components 0
end

# fix perms

file '/usr/local/bin/tzbuddy' do
  mode '0755'
  owner 'root'
  group 'root'
end
