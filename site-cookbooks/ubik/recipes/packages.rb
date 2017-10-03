package 'btrfs-tools'
package 'compizconfig-settings-manager'
package 'dstat'
package 'exfat-fuse'
package 'exfat-utils'
package 'firefox-trunk'
package 'google-chrome-stable'
package 'google-talkplugin'
package 'htop'
package 'keepassx'
package 'network-manager-openvpn-gnome'
package 'openvpn'
package 'powertop'
package 'shellcheck'
package 'telegram'
package 'tmux'
package 'ttf-mscorefonts-installer'
package 'ufraw'
package 'unity-tweak-tool'

package 'gstreamer1.0-plugins-ugly'
package 'gstreamer1.0-libav'
package 'gstreamer1.0-pulseaudio'
package 'libcurl3'

package 'libappindicator1'

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
