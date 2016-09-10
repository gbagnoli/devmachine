package 'apt-transport-https'

apt_repository 'git' do
  uri 'ppa:git-core/ppa'
end

apt_repository 'neovim' do
  uri 'ppa:neovim-ppa/unstable'
end

apt_repository 'chrome' do
  uri 'http://dl.google.com/linux/chrome/deb/'
  distribution 'stable'
  components ['main']
  key 'https://dl-ssl.google.com/linux/linux_signing_key.pub'
  arch 'amd64'
end

apt_repository 'hangout' do
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

apt_repository 'syncthing' do
  uri 'http://apt.syncthing.net/'
  distribution 'syncthing'
  components ['release']
  key 'https://syncthing.net/release-key.txt'
end

apt_repository 'weechat' do
  uri 'https://weechat.org/ubuntu'
  distribution node['lsb']['codename']
  components ['main']
  keyserver 'pool.sks-keyservers.net'
  key '11E9DE8848F2B65222AA75B8D1820DB22A11534E'
end

apt_repository 'fasd' do
  uri 'ppa:aacebedo/fasd'
end
