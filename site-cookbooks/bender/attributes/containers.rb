# # MARVIN
# id must be unique. Used for ips etc
default['bender']['containers']['marvin']['id'] = 2
default['bender']['containers']['marvin']['image'] = 'ubuntu:18.04'
# ssh auto-forwarded from base_ssh + id => 22
# format is [[host_port, port], [host_port2, port2]] or nil
default['bender']['containers']['marvin']['forwarded_ports'] = [[32400, 32400]]
# if set, this will nat 1:1 the ipv6 address to the container
default['bender']['containers']['marvin']['external_ipv6'] = nil
