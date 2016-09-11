remote_file '/var/chef/cache/suldr_keyring_1_all.deb' do
  source 'http://www.bchemnet.com/suldr/pool/debian/extra/su/suldr-keyring_1_all.deb'
  checksum '018fc0a97f9beb872f1075fd44903164fd7dbf29a7836b3dbb6d1801641c56e3'
  notifies :run, 'execute[install suldr key]', :immediately
end

execute 'install suldr key' do
  command 'dpkg -i /var/chef/cache/suldr_keyring_1_all.deb'
  action :nothing
end

apt_repository 'suldr' do
  uri 'http://www.bchemnet.com/suldr/'
  distribution 'debian'
  components ['extra']
end

package 'suld-driver-4.01.17'
package 'suld-configurator-2-qt4'
