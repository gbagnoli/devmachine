default["rupik"]["local_network"] = "172.25.2.0/24"
default["rupik"]["address"] = "dhcp"
default["rupik"]["ipv6"] = true
default["rupik"]["netmask"] = nil
default["rupik"]["gateway"] = nil
default["rupik"]["dns-nameservers"] = nil

default["rupik"]["skip_mounts"] = false
default["rupik"]["storage"]["dev"] = "/dev/sda3"
default["rupik"]["storage"]["path"] = "/srv"
default["rupik"]["www"]["domain"] = nil
default["rupik"]["www"]["pihole_domain"] = nil
default["rupik"]["www"]["user_emails"] = []

default["rupik"]["lan"]["ipv4"]["network"] = "172.25.2.0/24"
default["rupik"]["lan"]["ipv4"]["addr"] = "172.25.2.253"
default["rupik"]["podman"]["ipv4"]["network"] = "172.26.26.0/24"
default["rupik"]["podman"]["ipv4"]["addr"] = "172.26.26.1"
default["rupik"]["podman"]["ipv6"]["network"] = "fd59:4e23:2950:11f5::0/64"
default["rupik"]["podman"]["ipv6"]["addr"] = "fd59:4e23:2950:11f5::1"
