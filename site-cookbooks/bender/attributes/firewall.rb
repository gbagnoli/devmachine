default["bender"]["firewall"]["ipv4"]["open_ports"][22] = %w(tcp)
default["bender"]["firewall"]["ipv4"]["open_ports"][80] = %w(tcp)
default["bender"]["firewall"]["ipv4"]["open_ports"][443] = %w(tcp)
default["bender"]["firewall"]["ipv4"]["dnat_rules"] = {
 # example
   # "rulename" => {
   #   'local_ip': '172.24.24.1',
   #   'local_port': '22',
   #   'external_port' '2222',  # optional, defaults to local_port
   #   'proto': 'tcp',  # optional, defaults to 'tcp'
   #   'external_ip': <public_ip> or nil
   # },
  }

# and again, for ipv6
default["bender"]["firewall"]["ipv6"]["open_ports"][22] = %w(tcp)
default["bender"]["firewall"]["ipv6"]["open_ports"][80] = %w(tcp)
default["bender"]["firewall"]["ipv6"]["open_ports"][443] = %w(tcp)
default["bender"]["firewall"]["ipv6"]["dnat_rules"] = {}
default["bender"]["firewall"]["ipv6"]["nat"] = {}
