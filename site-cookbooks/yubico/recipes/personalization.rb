include_recipe 'yubico::ppa'

package 'yubikey-personalization'
package 'yubikey-personalization-gui'

service 'udev' do
  action :nothing
end

remote_file '/etc/udev/rules.d/70-u2f.rules' do
  source 'https://raw.githubusercontent.com/Yubico/libu2f-host/master/70-u2f.rules'
  notifies :restart, 'service[udev]', :immediately
end
