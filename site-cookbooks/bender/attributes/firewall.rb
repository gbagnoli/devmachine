default['firewall']['open_ports'] = [22]
default['firewall']['dnat_rules'] = [
  # example
  # {
  #   'local_ip': '172.24.24.1',
  #   'local_port': '22',
  #   'external_port' '2222',  # optional, defaults to local_port
  #   'proto': 'tcp',  # optional, defaults to 'tcp'
  # },
]
