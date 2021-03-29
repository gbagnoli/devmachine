def_ipv4 = node["bender"]["network"]["host"]["ipv4"]["addrs"][0]
def_ipv6 = node["bender"]["network"]["host"]["ipv6"]["addrs"][0]
bs_ipv4 = node["bender"]["network"]["host"]["ipv4"]["addrs"][1]
bs_ipv6 = node["bender"]["network"]["host"]["ipv6"]["addrs"][1]

# MARVIN
# id must be unique. Used for ips etc
default["bender"]["containers"]["marvin"]["id"] = 2
default["bender"]["containers"]["marvin"]["image"] = "ubuntu:18.04"
# ssh auto-forwarded from base_ssh + id => 22
# format:
# [{internal_port: x, external_port: y, ip_version: <ipv4|ipv6|all>, protocol: <tcp|udp|all>}, {}]
default["bender"]["containers"]["marvin"]["forwarded_ports"] = [
  { protocol: "udp", external_port: 1195, internal_port: 1194, ip_version: "all" },
  { protocol: "tcp", external_port: 22_000, internal_port: 22_000, ip_version: "all",
    external_ipv4: def_ipv4, external_ipv6: def_ipv6 },
]
# if set, this will nat 1:1 the ipv6 address to the container
default["bender"]["containers"]["marvin"]["external_ipv6"] = nil
default["bender"]["containers"]["marvin"]["volumes"] = nil

# FLEXO
default["bender"]["containers"]["flexo"]["id"] = 3
default["bender"]["containers"]["flexo"]["image"] = "ubuntu:18.04"
default["bender"]["containers"]["flexo"]["forwarded_ports"] = [
  { protocol: "tcp", internal_port: 32_400, external_port: 32_400, ip_version: "all" },
]
default["bender"]["containers"]["flexo"]["external_ipv6"] = nil
default["bender"]["containers"]["flexo"]["volumes"] = [
  {
    "name" => "media",
    "pool" => node["bender"]["storage"]["containers"]["name"],
    "source" => "media", # volume name in pool
    "path" => "/media",
    "type" => "disk",
  },
]

# PENTOLINO
default["bender"]["containers"]["pentolino"]["id"] = 4
default["bender"]["containers"]["pentolino"]["image"] = "ubuntu:18.04"
default["bender"]["containers"]["pentolino"]["forwarded_ports"] = []
default["bender"]["containers"]["pentolino"]["external_ipv6"] = nil
default["bender"]["containers"]["pentolino"]["volumes"] = nil
default["bender"]["containers"]["pentolino"]["action"] = :delete

# BLOODSEA
default["bender"]["containers"]["bloodsea"]["id"] = 5
default["bender"]["containers"]["bloodsea"]["image"] = "ubuntu:18.04"
default["bender"]["containers"]["bloodsea"]["forwarded_ports"] = [
  { protocol: "tcp", external_port: 22_000, internal_port: 22_000, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
  { protocol: "udp", external_port: 1194, internal_port: 1194, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
]
default["bender"]["containers"]["bloodsea"]["external_ipv6"] = nil
default["bender"]["containers"]["bloodsea"]["volumes"] = nil

# BEELZEBOT
default["bender"]["containers"]["beelzebot"]["id"] = 6
default["bender"]["containers"]["beelzebot"]["image"] = "ubuntu:18.04"
default["bender"]["containers"]["beelzebot"]["forwarded_ports"] = [27_960].map do |port|
  { protocol: "all", external_port: port, internal_port: port, ip_version: "all" }
end
default["bender"]["containers"]["beelzebot"]["external_ipv6"] = nil
default["bender"]["containers"]["beelzebot"]["volumes"] = nil
