default["calculon"]["rocky"]["btfrs_progs"] = {
  version: "6.8-1.el9",
  url: "https://cbs.centos.org/kojifiles/packages/btrfs-progs",
  packages: %w{btrfs-progs btrfs-progs-devel libbtrfs libbtrfsutil python3-btrfsutil}
}

default["calculon"]["rocky"]["podman"]["git"] = {
  url: "https://github.com/containers/podman.git",
  tag: "v5.0.1",
  # btrfs-devel is already installed from downloaded rpm in rocky
  deps:  %w{go systemd-devel gpgme-devel libseccomp-devel},
}

# rubocop:disable Layout/LineLength
default["calculon"]["rocky"]["crun"]["git"] = {
  url: "https://github.com/containers/crun.git",
  tag: "1.14.4",
  deps: %w{make automake autoconf gettext libtool gcc libcap-devel systemd-devel yajl-devel glibc-static libseccomp-devel},
}
# rubocop:enable Layout/LineLength


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
