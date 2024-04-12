default["calculon"]["rocky"]["btfrs_progs"] = {
  version: "6.8-1.el9",
  url: "https://cbs.centos.org/kojifiles/packages/btrfs-progs",
  packages: %w{btrfs-progs btrfs-progs-devel libbtrfs libbtrfsutil python3-btrfsutil}
}

default["calculon"]["data"]["username"] = "media"
default["calculon"]["data"]["group"] = "data"
default["calculon"]["data"]["uid"] = "2001"
default["calculon"]["data"]["gid"] = "2001"

default["calculon"]["storage"]["manage"] = true
default["calculon"]["storage"]["dev"] = "/dev/sda5"
default["calculon"]["storage"]["paths"]["root"] = "/var/lib/data"
default["calculon"]["storage"]["paths"]["sync"] = "/var/lib/data/sync"
default["calculon"]["storage"]["paths"]["media"] = "/var/lib/data/media"
default["calculon"]["storage"]["paths"]["downloads"] = "/var/lib/data/media/downloads"
default["calculon"]["storage"]["paths"]["library"] = "/var/lib/data/media/library"
default["calculon"]["storage"]["library_dirs"] = %w{movies series}

default["calculon"]["containers"]["storage"]["volume"] = "/var/lib/data/containers"
default["calculon"]["containers"]["storage"]["driver"] = "btrfs"
default["calculon"]["containers"]["storage"]["runroot"] = "/var/lib/data/containers/run"
default["calculon"]["containers"]["storage"]["graphroot"] = "/var/lib/data/containers/graph"

default["calculon"]["network"]["containers"]["ipv4"]["addr"] = "172.25.25.1"
default["calculon"]["network"]["containers"]["ipv4"]["addr_cidr"] = "172.25.25.1/24"
default["calculon"]["network"]["containers"]["ipv4"]["network"] = "172.25.25.0/24"
default["calculon"]["network"]["containers"]["ipv6"]["addr"] = "fd05:f439:6192:ffff::1"
default["calculon"]["network"]["containers"]["ipv6"]["addr_cidr"] = "fd05:f439:6192:ffff::1/64"
default["calculon"]["network"]["containers"]["ipv6"]["network"] = "fd05:f439:6192:ffff::0/64"