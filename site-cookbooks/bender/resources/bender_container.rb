resource_name :bender_container

id_callbacks = {
  'should be a value between 2 and 254' => lambda { |id|
    id > 1 && id < 255
  }
}

ports_callbacks = {
  'should be an hash with external_port, internal_port, protocol, ip_version' => lambda { |ports|
    ports.to_a.map do |pair|
      pair.key?('external_port') && \
        pair.key?('internal_port') && \
        pair.key?('protocol') && \
        pair.key?('ip_version')
    end.all?
  },
  'ports should be between 1 and 65536' => lambda { |ports|
    ports.to_a.map do |pair|
      [pair['external_port'], pair['internal_port']].each do |p|
        p.is_a?(Integer) && p >= 1 && p <= 65_536
      end.all?
    end.all?
  },
  'ip_version should be either "ipv4", "ipv6" or "all"' => lambda { |ports|
    ports.to_a.map do |pair|
      %w[ipv4 ipv6 all].include?(pair['ip_version'].to_s)
    end.all?
  },
  'protocol should be either "tcp", "udp" or "all"' => lambda { |ports|
    ports.to_a.map do |pair|
      %w[tcp udp all].include?(pair['protocol'].to_s)
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

allowed_volume_keys = %w[
  limits.read limits.write limits.max
  path source optional readonly recursive pool propagation
].sort

volumes_callbacks = {
  'it must be an hash with "source" and "path" required attribute' => lambda { |volumes|
    volumes.each do |volume|
      volume.is_a?(Hash) && volume.key?('source') && volume.key?('path')
    end.all?
  },
  'it must have type set to "disk"' => lambda { |volumes|
    volumes.each do |volume|
      volume.is_a?(Hash) && volume['type'] == 'disk'
    end.all?
  },
  'source directory must be a directory' => lambda { |volumes|
    volumes.each do |volume|
      ::File.directory?(::File.realpath(volume['source'])) && \
        (volume['pool'].nil? || node['bender']['storage'].key?(volume['pool']))
    end.all?
  },
  'all volumes must have a unique name' => lambda { |volumes|
    names = volumes.map { |v| v['name'] }
    names.all? && names.uniq.length == names.length
  },
  'volumes should have only keys from configuration' => lambda { |volumes|
    volumes.each do |vol|
      (vol.keys - allowed_volume_keys).empty?
    end.all?
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
property :volumes, Array, default: [], callbacks: volumes_callbacks
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

  directory "#{new_resource.container_name}_cert_vol_path" do
    path ssl_cert_directory
    mode '0700'
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
      bridge_interface: node['bender']['network']['containers']['interface'],
      volumes: volumes
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

  new_resource.forwarded_ports.each do |portdesc|
    protocols = portdesc['protocol'].to_s == 'all' ? %i[tcp udp] : [portdesc['protocol'].to_sym]
    versions = portdesc['ip_version'].to_s == 'all' ? %i[ipv4 ipv6] : [portdesc['ip_version'].to_sym]
    external = portdesc['external_port']
    internal = portdesc['internal_port']

    protocols.each do |proto|
      versions.each do |ipv|
        rulename = "#{new_resource.container_name}_#{proto}_#{external}_#{internal}"
        node.override['bender']['firewall'][ipv]['dnat_rules'][rulename] = create_rule(
          ipv, external, internal, proto
        )
      end
    end
  end
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

    check = ssh_check('ipv4')
    node.override['bender']['tcp_checks'][check[:name]] = check
  end

  unless get_ipv6_address(new_resource.container_name).empty?
    hostsfile_entry get_ipv6_address(new_resource.container_name) do
      hostname "#{new_resource.container_name}.lxd"
    end
    check = ssh_check('ipv6')
    node.override['bender']['tcp_checks'][check[:name]] = check
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

  def get_ip(ip_version)
    if ip_version.to_s == 'ipv4'
      get_ipv4_address(new_resource.container_name)
    elsif ip_version.to_s == 'ipv6'
      get_ipv6_address(new_resource.container_name)
    else
      raise ArgumentError, "#{ip_version} not recognized"
    end
  end

  def create_rule(ip_version, external_port, internal_port, protocol)
    ip = get_ip(ip_version)
    {
      'local_ip': ip,
      'local_port': internal_port,
      'external_port': external_port,
      'proto': protocol.to_s
    }
  end

  def ssh_check(ip_version)
    {
      name: "#{new_resource.container_name}_ssh_#{ip_version}",
      host: get_ip(ip_version),
      port: 22
    }
  end

  def ssh_rule_v4
    create_rule(:ipv4, ssh_port, 22, :tcp)
  end

  def ssh_rule_v6
    create_rule(:ipv6, ssh_port, 22, :tcp)
  end

  def ssl_cert_directory
    "#{node['bender']['certificates']['directory']}/#{new_resource.container_name}"
  end

  def ssl_cert_volume
    {
      'name' => "#{new_resource.container_name}_certificates",
      'source' => ssl_cert_directory,
      'path' => ssl_cert_directory,
      'readonly' => true,
      'type' => 'disk'
    }
  end

  def volumes
    # append the certificate directory, read only
    vol = [ssl_cert_volume]
    new_resource.volumes.each { |v| vol << v }
    # sort them by name to keep profile generation stable
    vol.sort_by { |v| v['name'] }
  end
end
