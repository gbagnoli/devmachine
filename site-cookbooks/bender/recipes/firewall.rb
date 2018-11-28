package 'shorewall'

include_recipe 'sysctl'

sysctl_param 'net.ipv4.ip_forward' do
  value 1
end

%w[policy zones].each do |shore_file|
  cookbook_file "/etc/shorewall/#{shore_file}" do
    action  :create
    owner   'root'
    group   'root'
    mode    '0644'
    source  "shorewall/#{shore_file}"
    notifies  :restart, 'service[shorewall]'
  end
end

cookbook_file '/etc/default/shorewall' do
  action  :create
  owner   'root'
  group   'root'
  mode    '0644'
  source  'shorewall/default'
  notifies :restart, 'service[shorewall]'
end

%w[rules interfaces masq].each do |shore_template|
  template "/etc/shorewall/#{shore_template}" do
    action    :create
    owner     'root'
    group     'root'
    mode      '0644'
    source    "shorewall/#{shore_template}.erb"
    notifies  :restart, 'service[shorewall]'
    variables(
      firewall: node['bender']['firewall'],
      default_interface: node['bender']['network']['host']['interface'],
      lxc_interface: node['bender']['network']['containers']['interface']
    )
  end
end

service 'shorewall' do
  action 'start'
end
