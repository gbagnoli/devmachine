default['bender']['network']['host']['interface'] = 'enp4s0'
default['bender']['network']['host']['ipv4']['addr'] = '144.76.31.236/32'
default['bender']['network']['host']['ipv6']['addr'] = '2a01:4f8:191:22eb::2/64'
default['bender']['network']['containers']['interface'] = 'lxdbr0'
default['bender']['network']['containers']['ipv4']['addr'] = '172.24.24.1/24'
default['bender']['network']['containers']['ipv4']['network'] = '172.24.24.0/24'
default['bender']['network']['containers']['ipv6']['addr'] = 'fd05:f439:6192:1b03::1/64'
default['bender']['network']['containers']['ipv6']['network'] = 'fd05:f439:6192:1b03::0/64'
default['bender']['storage']['containers']['name'] = 'data'
default['bender']['storage']['containers']['source'] = '/data/containers'
default['bender']['storage']['containers']['driver'] = 'btrfs'

default['bender']['lxd']['config'] = <<~HEREDOC
  config:
    images.auto_update_interval: 15
  networks:
  - name: #{node['bender']['network']['containers']['interface']}
    type: bridge
    config:
      ipv4.address: #{node['bender']['network']['containers']['ipv4']['addr']}
      ipv6.address: #{node['bender']['network']['containers']['ipv6']['addr']}

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
      eth0:
          name: eth0
          nictype: bridged
          parent: #{node['bender']['network']['containers']['interface']}
          type: nic
HEREDOC
