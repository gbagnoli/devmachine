{
  'shorewall' => 'ipv4',
  'shorewall6' => 'ipv6'
}.each do |pkg, protocol|
  package pkg

  %w[policy].each do |shore_file|
    cookbook_file "/etc/#{pkg}/#{shore_file}" do
      action  :create
      owner   'root'
      group   'root'
      mode    '0644'
      source  "shorewall/#{shore_file}"
      notifies  :restart, "service[#{pkg}]"
    end
  end

  cookbook_file "/etc/default/#{pkg}" do
    action  :create
    owner   'root'
    group   'root'
    mode    '0644'
    source  'shorewall/default'
    notifies :restart, "service[#{pkg}]"
  end

  %w[zones rules interfaces masq nat].each do |shore_template|
    next if shore_template == 'nat' && protocol != 'ipv6'

    template "/etc/#{pkg}/#{shore_template}" do
      action    :create
      owner     'root'
      group     'root'
      mode      '0644'
      source    "shorewall/#{shore_template}.erb"
      notifies  :restart, "service[#{pkg}]"
      variables(
        protocol: protocol,
        firewall: lazy { node['bender']['firewall'][protocol] },
        default_interface: node['bender']['network']['host']['interface'],
        lxc_interface: node['bender']['network']['containers']['interface'],
        network: node['bender']['network']['containers'][protocol]['network']
      )
    end
  end

  service pkg do
    action 'start'
  end
end
