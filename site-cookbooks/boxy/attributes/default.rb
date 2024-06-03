default["boxy"]["local_network"] = "172.25.2.0/24"
default["boxy"]["address"] = "dhcp"
default["boxy"]["ipv6"] = true
default["boxy"]["netmask"] = nil
default["boxy"]["gateway"] = nil
default["boxy"]["dns-nameservers"] = nil

default["boxy"]["skip_mounts"] = false
default["boxy"]["storage"]["dev"] = "/dev/sda3"
default["boxy"]["storage"]["path"] = "/srv"
default["boxy"]["www"]["domain"] = nil
default["boxy"]["www"]["pihole_domain"] = nil
default["boxy"]["www"]["user_emails"] = []

default["boxy"]["lan"]["ipv4"]["network"] = "172.25.2.0/24"
default["boxy"]["lan"]["ipv4"]["addr"] = "172.25.2.254"
default["boxy"]["podman"]["ipv4"]["network"] = "172.26.27.0/24"
default["boxy"]["podman"]["ipv4"]["addr"] = "172.26.27.1"
default["boxy"]["podman"]["ipv6"]["network"] = "fd03:3fce:1f4b:8dd8::0/64"
default["boxy"]["podman"]["ipv6"]["addr"] = "fd03:3fce:1f4b:8dd8::1"
