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

remote_file '/usr/src/viber.deb' do
  source 'http://download.cdn.viber.com/cdn/desktop/Linux/viber.deb'
  notifies :run, 'execute[install_viber]', :immediately
end

execute 'install_viber' do
  action :nothing
  command 'dpkg -i /usr/src/viber.deb'
end

remote_file '/usr/src/skype.deb' do
  source 'https://repo.skype.com/latest/skypeforlinux-64.deb'
  notifies :run, 'execute[install_skype]', :immediately
end

execute 'install_skype' do
  action :nothing
  command 'dpkg -i /usr/src/skype.deb'
end
