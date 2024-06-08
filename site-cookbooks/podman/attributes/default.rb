# rubocop:disable Layout/LineLength
default["podman"]["go"]["version"] = "1.22.3"
default["podman"]["sources"]["podman"] = {
  url: "https://github.com/containers/podman.git",
  tag: "v5.0.1",
  # btrfs-devel is already installed from downloaded rpm in rocky
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{go systemd-devel gpgme-devel libseccomp-devel ostree-devel shadow-utils-subid-devel},
    "debian"=> %w{}
  ),
  download: [{
    url: "https://kojipkgs.fedoraproject.org//packages/containers-common/1/95.fc39/noarch",
    rpms: %w{containers-common-1-95.fc39.noarch.rpm containers-common-extra-1-95.fc39.noarch.rpm}
  }]
}

default["podman"]["sources"]["crun"] = {
  url: "https://github.com/containers/crun.git",
  tag: "1.14.4",
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{make automake autoconf gettext libtool gcc libcap-devel systemd-devel yajl-devel glibc-static libseccomp-devel},
    "debian"=> %w{make git gcc build-essential pkgconf libtool libsystemd-dev libprotobuf-c-dev libcap-dev libseccomp-dev libyajl-dev go-md2man autoconf python3 automake}
  ),
}

default["podman"]["sources"]["conmon"] = {
  url: "https://github.com/containers/conmon.git",
  tag: "v2.1.12",
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{gcc git glib2-devel glibc-devel libseccomp-devel make pkgconfig runc},
    "debian"=> %w{gcc git libc6-dev libglib2.0-dev libseccomp-dev pkg-config make runc}
  ),
}

default["podman"]["sources"]["catatonit"] = {
  url: "https://github.com/openSUSE/catatonit.git",
  tag: "v0.2.0",
  deps:  value_for_platform_family(
    %w{fedora rhel} => [],
    "debian"=> [],
  ),
}
# rubocop:enable Layout/LineLength
