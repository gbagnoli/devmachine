# frozen_string_literal: true

package 'dirmngr'
package 'apt-transport-https'

execute 'enable 32 bit arch' do
  command 'dpkg --add-architecture i386'
  not_if 'dpkg --print-foreign-architectures | grep -q i386'
  notifies :update, 'apt_update[after-arch-add]', :immediately
end

apt_update 'after-arch-add' do
  action :nothing
end

apt_repository 'git' do
  uri 'ppa:git-core/ppa'
end

file '/etc/apt/sources.list.d/chrome.list' do
  action :delete
end

file '/etc/apt/sources.list.d/hangout.list' do
  action :delete
end

apt_repository 'google-chrome' do
  uri 'http://dl.google.com/linux/chrome/deb/'
  distribution 'stable'
  components ['main']
  key 'https://dl-ssl.google.com/linux/linux_signing_key.pub'
  arch 'amd64'
end

apt_repository 'google-talkplugin' do
  uri 'http://dl.google.com/linux/talkplugin/deb/'
  distribution 'stable'
  components ['main']
  key 'https://dl-ssl.google.com/linux/linux_signing_key.pub'
end

apt_repository 'ubuntu-partner' do
  uri 'http://archive.canonical.com/'
  distribution node['lsb']['codename']
  components ['partner']
  arch 'i386'
end

apt_repository 'telegram' do
  uri 'ppa:atareao/telegram'
end

apt_package 'firefox-trunk' do
  action :purge
end

file '/etc/apt/sources.list.d/firefox-nightly.list' do
  action :delete
end

apt_repository 'firefox-beta' do
  uri 'ppa:mozillateam/firefox-next'
end

apt_repository 'docker' do
  uri 'https://download.docker.com/linux/ubuntu'
  arch 'amd64'
  distribution node['lsb']['codename']
  components ['stable']
  key 'https://download.docker.com/linux/ubuntu/gpg'
end

apt_repository 'signal' do
  uri 'https://updates.signal.org/desktop/apt'
  arch 'amd64'
  distribution 'xenial'
  components ['main']
  key 'https://updates.signal.org/desktop/apt/keys.asc'
end

if node['lsb']['codename'] == 'xenial'
  apt_repository 'gnome3-staging' do
    uri 'ppa:gnome3-team/gnome3-staging'
  end

  apt_repository 'gnome3' do
    uri 'ppa:gnome3-team/gnome3'
  end
end

apt_repository 'virtualbox' do
  uri 'https://download.virtualbox.org/virtualbox/debian'
  components ['contrib']
  distribution node['lsb']['codename']
  key 'https://www.virtualbox.org/download/oracle_vbox_2016.asc'
end

apt_repository 'wireguard' do
  uri 'ppa:wireguard/wireguard'
end
