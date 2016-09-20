package 'apt-transport-https'

releases = {
  security: { prio: 1000, pin: 'release l=Debian-Security' },
  stable: { prio: 900, pin: 'release a=stable' },
  testing: { prio: 750, pin: 'release a=testing' },
  unstable: { prio: 50, pin: 'release a=unstable' },
  experimental: { prio: 1, pin: 'release a=experimental' },
  'ubilinux-upboard' => { prio: 1001, pin: 'release a=ubilinux3-upboard' }
}

mirror = 'http://debian.heanet.ie/debian/'
def_components = %w(main contrib non-free)

releases.each do |name, conf|
  apt_preference name do
    glob '*'
    pin conf[:pin]
    pin_priority conf[:prio].to_s
  end
end

%w(ubilinux-archive-jessie-ubilinux.gpg ubiworx-archive-ubiworx.gpg).each do |f|
  cookbook_file "/etc/apt/trusted.gpg.d/#{f}" do
    source f
    mode '0644'
  end
end

%w(jessie jessie-security jessie-updates sources).each do |repo|
  apt_repository repo do
    action :remove
  end
end

apt_repository 'ubilinux' do
  uri 'http://ubilinux.org/ubilinux'
  distribution 'ubilinux3-upboard'
  components ['main']
end

apt_repository 'ubiworx' do
  uri 'http://ubiworx.com/debian'
  distribution 'wheezy'
  components ['non-free']
end

apt_repository 'stable-security' do
  uri 'http://security.debian.org/'
  distribution 'stable/updates'
  components def_components
end

apt_repository 'testing-security' do
  uri 'http://security.debian.org/'
  distribution 'testing/updates'
  components def_components
end

%w(stable testing unstable experimental).each do |distribution|
  apt_repository distribution do
    uri mirror
    distribution distribution
    components def_components
  end
end

%w(stable testing).each do |distribution|
  apt_repository "#{distribution}-updates" do
    uri mirror
    distribution "#{distribution}-updates"
    components def_components
  end
end

apt_repository 'jessie-backports' do
  uri 'http://httpredir.debian.org/debian'
  distribution 'jessie-backports'
  components def_components
end
