config = <<~HEREDOC
  config:
    images.auto_update_interval: 15
  networks:
  - name: #{node['bender']['network']['containers']['interface']}
    type: bridge
    config:
      ipv4.address: #{node['bender']['network']['containers']['ipv4']['addr']}
      ipv6.address: #{node['bender']['network']['containers']['ipv6']['addr']}
      ipv4.firewall: false
      ipv6.firewall: false
      ipv6.dhcp.stateful: true

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

node['bender']['containers'].each do |name, conf|
  bender_container name do
    id conf['id']
    image conf['image']
    description conf['description']
    forwarded_ports conf['forwarded_ports']
    external_ipv6 conf['external_ipv6']
  end
end
