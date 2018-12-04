# # MARVIN
# id must be unique. Used for ips etc
default['bender']['containers']['marvin']['id'] = 2
default['bender']['containers']['marvin']['image'] = 'ubuntu:18.04'
# ssh auto-forwarded from base_ssh + id => 22
# format is [[host_port, port, proto], [host_port2, port2, proto]] or nil
default['bender']['containers']['marvin']['forwarded_ports'] = [[1194, 1194, :udp]]
# if set, this will nat 1:1 the ipv6 address to the container
default['bender']['containers']['marvin']['external_ipv6'] = nil

default['bender']['containers']['flexo']['id'] = 3
default['bender']['containers']['flexo']['image'] = 'ubuntu:18.04'
default['bender']['containers']['flexo']['forwarded_ports'] = [[32_400, 32_400, :tcp]]
default['bender']['containers']['flexo']['external_ipv6'] = nil
