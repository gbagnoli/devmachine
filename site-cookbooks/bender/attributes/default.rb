default['bender']['network']['host']['interface'] = 'enp4s0'
default['bender']['network']['host']['ipv4']['addr'] = '144.76.31.236/32'
default['bender']['network']['host']['ipv6']['addr'] = '2a01:4f8:191:22eb::2/64'

default['bender']['network']['containers']['interface'] = 'lxdbr0'

default['bender']['network']['containers']['base_ssh'] = 3000
default['bender']['network']['containers']['ipv4']['addr'] = '172.24.24.1'
default['bender']['network']['containers']['ipv4']['addr_cidr'] = '172.24.24.1/24'
default['bender']['network']['containers']['ipv4']['network'] = '172.24.24.0/24'
default['bender']['network']['containers']['ipv6']['addr'] = 'fd05:f439:6192:1b03::1'
default['bender']['network']['containers']['ipv6']['addr_cidr'] = 'fd05:f439:6192:1b03::1/64'
default['bender']['network']['containers']['ipv6']['network'] = 'fd05:f439:6192:1b03::0/64'
default['bender']['storage']['containers']['name'] = 'data'
default['bender']['storage']['containers']['source'] = '/data/containers'
default['bender']['storage']['containers']['driver'] = 'btrfs'

# id must be unique. Used for ips etc
default['bender']['containers']['marvin']['id'] = 2
default['bender']['containers']['marvin']['image'] = 'ubuntu:18.04'
# ssh auto-forwarded from base_ssh + id => 22
# format is [[host_port, port], [host_port2, port2]] or nil
default['bender']['containers']['marvin']['forwarded_ports'] = nil
# if set, this will nat 1:1 the ipv6 address to the container
default['bender']['containers']['marvin']['external_ipv6'] = nil
