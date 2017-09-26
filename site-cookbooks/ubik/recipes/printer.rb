directory '/var/chef/cache' do
  action :create
  recursive true
end

remote_file '/var/chef/cache/suldr_keyring_2_all.deb' do
  source 'http://www.bchemnet.com/suldr/pool/debian/extra/su/suldr-keyring_2_all.deb'
  checksum '2d996f611648a1a0a2926ceea1493ce1f29f5c1ee9ed604c61f60b87856339ae'
  notifies :run, 'execute[install suldr key]', :immediately
end

execute 'install suldr key' do
  command 'dpkg -i /var/chef/cache/suldr_keyring_2_all.deb'
  action :nothing
end

apt_repository 'suldr' do
  uri 'http://www.bchemnet.com/suldr/'
  distribution 'debian'
  components ['extra']
end

package 'suld-driver-4.01.17'
