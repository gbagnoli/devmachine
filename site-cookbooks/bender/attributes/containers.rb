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
  # openvpn
  { protocol: "udp", external_port: 1195, internal_port: 1194, ip_version: "all" },
  # syncthing
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
  # plex
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

# PENTOLINO (retired)  - id 4

# BLOODSEA
default["bender"]["containers"]["bloodsea"]["id"] = 5
default["bender"]["containers"]["bloodsea"]["image"] = "ubuntu:18.04"
default["bender"]["containers"]["bloodsea"]["forwarded_ports"] = [
  # syncthing
  { protocol: "tcp", external_port: 22_000, internal_port: 22_000, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
  # openvpn
  { protocol: "udp", external_port: 1194, internal_port: 1194, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
]
default["bender"]["containers"]["bloodsea"]["external_ipv6"] = nil
default["bender"]["containers"]["bloodsea"]["volumes"] = nil

# BEELZEBOT (retired) - id 6

# WHITESTONE
default["bender"]["containers"]["whitestone"]["id"] = 7
default["bender"]["containers"]["whitestone"]["image"] = "ubuntu:20.04"
default["bender"]["containers"]["whitestone"]["forwarded_ports"] = [
  # minecraft
  { protocol: "tcp", external_port: 25_565, internal_port: 25_565, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
  { protocol: "udp", external_port: 19_132, internal_port: 19_132, ip_version: "all",
    external_ipv4: bs_ipv4, external_ipv6: bs_ipv6 },
]
default["bender"]["containers"]["whitestone"]["external_ipv6"] = nil
default["bender"]["containers"]["whitestone"]["volumes"] = nil
