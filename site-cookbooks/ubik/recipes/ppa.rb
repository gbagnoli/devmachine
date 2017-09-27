package 'apt-transport-https'

apt_repository 'git' do
  uri 'ppa:git-core/ppa'
end

apt_repository 'neovim' do
  uri 'ppa:neovim-ppa/unstable'
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

apt_repository 'fasd' do
  uri 'ppa:aacebedo/fasd'
end

apt_repository 'telegram' do
  uri 'ppa:atareao/telegram'
end

apt_repository 'firefox-nightly' do
  uri 'ppa:ubuntu-mozilla-daily/ppa'
end
