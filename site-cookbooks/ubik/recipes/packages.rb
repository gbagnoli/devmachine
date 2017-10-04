packages = %w[
  btrfs-tools compizconfig-settings-manager dstat exfat-fuse
  exfat-utils firefox-trunk google-chrome-stable
  google-talkplugin htop keepassx network-manager-openvpn-gnome
  openvpn powertop shellcheck telegram tmux
  ttf-mscorefonts-installer ufraw unity-tweak-tool
  gstreamer1.0-plugins-ugly gstreamer1.0-libav
  gstreamer1.0-pulseaudio libcurl3 libappindicator1
  docker-ce python-apt gnome-terminal
  qemu-kvm libvirt-dev libxslt1-dev libxml2-dev
]

package 'base install' do
  package_name packages
  action :install
end

packages = {
  'viber' => 'http://download.cdn.viber.com/cdn/desktop/Linux/viber.deb',
  'skype' => 'https://repo.skype.com/latest/skypeforlinux-64.deb',
  'keybase' => 'https://prerelease.keybase.io/keybase_amd64.deb',
  'steam' => 'https://steamcdn-a.akamaihd.net/client/installer/steam.deb',
  'vagrant' => 'https://releases.hashicorp.com/vagrant/2.0.0/vagrant_2.0.0_x86_64.deb'
}

# accept steam license
debconf_selection 'steam/question' do
  value 'I AGREE'
  package 'steam'
  type 'select'
end

debconf_selection 'steam/license' do
  value ''
  type 'note'
  package 'steam'
end

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

vagrant_plugins = %w[
  vagrant-libvirt
  vagrant-kvm
]

vagrant_plugins.each do |plg|
  execute "vagrant_install_#{plg}" do
    command "vagrant plugin install #{plg}"
    not_if "vagrant plugin list | grep -q '^#{plg} '"
  end
end
