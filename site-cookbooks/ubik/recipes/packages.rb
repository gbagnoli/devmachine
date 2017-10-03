packages = %w[
  btrfs-tools compizconfig-settings-manager dstat exfat-fuse
  exfat-utils firefox-trunk google-chrome-stable
  google-talkplugin htop keepassx network-manager-openvpn-gnome
  openvpn powertop shellcheck telegram tmux
  ttf-mscorefonts-installer ufraw unity-tweak-tool
  gstreamer1.0-plugins-ugly gstreamer1.0-libav
  gstreamer1.0-pulseaudio libcurl3 libappindicator1
]

package 'base install' do
  package_name packages
  action :install
end

packages = {
  'viber' => 'http://download.cdn.viber.com/cdn/desktop/Linux/viber.deb',
  'skype' => 'https://repo.skype.com/latest/skypeforlinux-64.deb',
  'keybase' => 'https://prerelease.keybase.io/keybase_amd64.deb'
}

packages.each do |name, url|
  debfile = "/usr/src/#{name}.deb"
  remote_file debfile do
    source url
    notifies :run, "execute[install_#{name}]", :immediately
  end

  execute "install_#{name}" do
    action :nothing
    command "dpkg -i #{debfile}"
  end
end
