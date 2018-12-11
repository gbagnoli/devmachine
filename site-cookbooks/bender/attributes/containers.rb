# # MARVIN
# id must be unique. Used for ips etc
default['bender']['containers']['marvin']['id'] = 2
default['bender']['containers']['marvin']['image'] = 'ubuntu:18.04'
# ssh auto-forwarded from base_ssh + id => 22
# format:
# [{internal_port: x, external_port: y, ip_version: <ipv4|ipv6|all>, protocol: <tcp|udp|all>}, {}]
default['bender']['containers']['marvin']['forwarded_ports'] = [
  { protocol: 'udp', external_port: 1195, internal_port: 1194, ip_version: 'all' }
]
# if set, this will nat 1:1 the ipv6 address to the container
default['bender']['containers']['marvin']['external_ipv6'] = nil

default['bender']['containers']['flexo']['id'] = 3
default['bender']['containers']['flexo']['image'] = 'ubuntu:18.04'
default['bender']['containers']['flexo']['forwarded_ports'] = [
  { protocol: 'tcp', internal_port: 32_400, external_port: 32_400, ip_version: 'all' }
]
default['bender']['containers']['flexo']['external_ipv6'] = nil
