config = <<~HEREDOC
  config:
    images.auto_update_interval: 15
  networks:
  - name: #{node['bender']['network']['containers']['interface']}
    type: bridge
    config:
      ipv4.address: #{node['bender']['network']['containers']['ipv4']['addr_cidr']}
      ipv6.address: #{node['bender']['network']['containers']['ipv6']['addr_cidr']}
      ipv4.firewall: false
      ipv6.firewall: false
      ipv6.dhcp.stateful: true
      raw.dnsmasq: |
        auth-zone=lxd
        dns-loop-detect

  storage_pools:
  - name: #{node['bender']['storage']['containers']['name']}
    driver: #{node['bender']['storage']['containers']['driver']}
    config:
      source: #{node['bender']['storage']['containers']['source']}

  profiles:
  - name: default
    devices:
      root:
         path: /
         pool: #{node['bender']['storage']['containers']['name']}
         type: disk
HEREDOC

lxd_config 'bender' do
  content config
end

%w[lxd-host-dns-start lxd-host-dns-stop].each do |script|
  path = "/usr/local/bin/#{script}"
  template path do
    source "#{script}.erb"
    variables(
      interface: node['bender']['network']['containers']['interface'],
      address: node['bender']['network']['containers']['ipv4']['addr'],
      domain: 'lxd'
    )
    mode 0o750
  end
end

systemd_unit 'lxd-host-dns.service' do
  content <<~EOU
    [Unit]
    Description=LXD host DNS service
    After=multi-user.target
     [Service]
    Type=simple
    ExecStart=/usr/local/bin/lxd-host-dns-start
    RemainAfterExit=true
    ExecStop=/usr/local/bin/lxd-host-dns-stop
    StandardOutput=journal
     [Install]
    WantedBy=multi-user.target
  EOU
  action %i[create enable start]
end

node['bender']['containers'].each do |name, conf|
  bender_container name do
    id conf['id']
    image conf['image']
    description conf['description']
    forwarded_ports conf['forwarded_ports']
    external_ipv6 conf['external_ipv6']
  end
end
