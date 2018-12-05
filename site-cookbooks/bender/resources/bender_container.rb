resource_name :bender_container

id_callbacks = {
  'should be a value between 2 and 254' => lambda { |id|
    id > 1 && id < 255
  }
}

ports_callbacks = {
  'should be an array of two integers and a symbol' => lambda { |ports|
    ports.to_a.map do |pair|
      pair.length == 3 && pair[0..1].map { |p| p.is_a?(Integer) }.all? \
        && %i[tcp udp].include?(pair[2])
    end.all?
  },
  'ports should be between 1 and 65536' => lambda { |ports|
    ports.to_a.map do |pair|
      pair[0..1].map { |p| p >= 1 && p <= 65_536 }.all?
    end.all?
  }
}

ipv6_callbacks = {
  'should be a valid ipv6 address' => lambda { |ip|
    return true if ip.nil?

    require 'ipaddr'
    begin
      IPAddr.new(ip).ipv6?
    rescue IPAddr::InvalidAddressError
      false
    end
  }
}

property :container_name, String, name_property: true
property :id, Integer, required: true, callbacks: id_callbacks
property :image, String, required: true
property :description, [String, NilClass], default: ''
property :forwarded_ports, [Array, NilClass], callbacks: ports_callbacks, default: []
property :external_ipv6, [String, NilClass], default: nil, callbacks: ipv6_callbacks
# snapshots scheduling support is merged but not released yet as of 3/12/18
property :snapshots, [TrueClass, FalseClass], default: false
default_action :create

action :create do
  execute "create_profile_#{new_resource.container_name}" do
    # rubocop:disable LineLength
    command "(lxc profile list | grep -q ' #{new_resource.container_name} ') || lxc profile create #{new_resource.container_name}"
    # rubocop:enable LineLength

    action :nothing
  end

  execute "update_profile_#{new_resource.container_name}" do
    command "lxc profile edit #{new_resource.container_name} < #{profile_path}"
    action :nothing
  end

  template profile_path do
    source 'container_profile.erb'
    variables(
      name: new_resource.container_name,
      description: new_resource.description,
      autostart: true,
      autostart_delay: 10,
      snapshots_schedule: new_resource.snapshots ? "10 #{new_resource.id % 24} * * *" : nil,
      ipv4_addr: ipv4_addr,
      ipv6_addr: ipv6_addr,
      bridge_interface: node['bender']['network']['containers']['interface']
    )
    notifies :run, "execute[create_profile_#{new_resource.container_name}]", :immediately
    notifies :run, "execute[update_profile_#{new_resource.container_name}]", :immediately
  end

  execute "lxd_lauch_container_#{new_resource.container_name}" do
    # rubocop:disable LineLength
    command "lxc launch -p default -p #{new_resource.container_name} #{new_resource.image} #{new_resource.container_name}"
    not_if { ::File.exist?("/var/lib/lxd/containers/#{new_resource.container_name}/metadata.yaml") }
    notifies :run, "execute[copy_ssh_config_for_root_#{new_resource.container_name}]", :immediately
    # rubocop:enable LineLength
  end

  execute "copy_ssh_config_for_root_#{new_resource.container_name}" do
    # we need to sleep as we need to wait for the container to boot. This sucks, but it's enough for now

    # rubocop:disable LineLength
    command "sleep 30s && lxc file push /root/.ssh/authorized_keys #{new_resource.container_name}/root/.ssh/authorized_keys --mode 0400"
    # rubocop:enable LineLength
    action :nothing
  end

  # rubocop:disable LineLength
  node.override['bender']['firewall']['ipv4']['dnat_rules']["#{new_resource.container_name}_ssh"] = ssh_rule_v4
  node.override['bender']['firewall']['ipv4']['open_ports'][ssh_port] = %w[tcp]
  node.override['bender']['firewall']['ipv6']['dnat_rules']["#{new_resource.container_name}_v6_ssh"] = ssh_rule_v6
  node.override['bender']['firewall']['ipv6']['open_ports'][ssh_port] = %w[tcp]
  node.override['bender']['containers']['marvin']['ipv4_address'] = get_ipv4_address(new_resource.container_name)
  node.override['bender']['containers']['marvin']['ipv6_address'] = get_ipv6_address(new_resource.container_name)
  # rubocop:enable LineLength

  unless new_resource.external_ipv6.nil?
    # rubocop:disable LineLength
    node.override['bender']['firewall']['ipv6']['nat'][external_ipv6] = get_ipv6_address(new_resource.container_name)
    # rubocop:enable LineLength
  end

  unless get_ipv4_address(new_resource.container_name).empty?
    hostsfile_entry get_ipv4_address(new_resource.container_name) do
      hostname "#{new_resource.container_name}.lxd"
    end
  end

  unless get_ipv6_address(new_resource.container_name).empty?
    hostsfile_entry get_ipv6_address(new_resource.container_name) do
      hostname "#{new_resource.container_name}.lxd"
    end
  end
end

action_class do
  require 'ipaddr'
  include LXD::Container

  def profile_path
    "#{node['lxd']['config_dir']}/profile_#{new_resource.container_name}.yaml"
  end

  def get_addr(net)
    # ok this is clowny
    addr = net
    1.upto(new_resource.id) do
      addr = addr.succ
    end
    addr
  end

  def ipv4_addr
    get_addr(ipv4_net).to_s
  end

  def ipv6_addr
    get_addr(ipv6_net).to_s
  end

  def ipv4_net
    IPAddr.new((node['bender']['network']['containers']['ipv4']['network']).to_s)
  end

  def ipv6_net
    IPAddr.new((node['bender']['network']['containers']['ipv6']['network']).to_s)
  end

  def ssh_port
    node['bender']['network']['containers']['base_ssh'] + new_resource.id
  end

  def external_ipv6
    IPAddr.new(new_resource.external_ipv6).to_s
  end

  def ssh_rule_v4
    {
      'local_ip': get_ipv4_address(new_resource.container_name),
      'local_port': '22',
      'external_port': ssh_port,
      'proto': 'tcp'
    }
  end

  def ssh_rule_v6
    {
      'local_ip': get_ipv6_address(new_resource.container_name),
      'local_port': '22',
      'external_port': ssh_port,
      'proto': 'tcp'
    }
  end
end
