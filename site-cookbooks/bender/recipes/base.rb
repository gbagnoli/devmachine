include_recipe 'apt'

file '/etc/apt/apt.conf.d/local' do
  owner 'root'
  group 'root'
  mode 0o0644
  content 'Dpkg::Options {
   "--force-confdef";
   "--force-confold";
}'
  action :create
end

package 'base' do
  package_name %w[curl htop iotop iperf]
end
