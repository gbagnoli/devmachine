default["podman"]["go"]["version"] = "1.26.6"
default["podman"]["sources"]["podman"] = {
  url: "https://github.com/containers/podman.git",
  tag: "v6.1.0",
  # btrfs-devel is already installed from downloaded rpm in rocky
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{go systemd-devel gpgme-devel libseccomp-devel ostree-devel shadow-utils-subid-devel sqlite-devel rpmautospec-rpm-macros},
    "debian"=> %w{}
  ),
  rpms: [{
    url: "https://kojipkgs.fedoraproject.org/packages/containers-common/1/99.fc39/noarch/",
    rpms: %w{containers-common-1-99.fc39.noarch.rpm containers-common-extra-1-99.fc39.noarch.rpm}
  }]
}

default["podman"]["sources"]["crun"] = {
  url: "https://github.com/containers/crun.git",
  tag: "1.29.1",
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{make automake autoconf gettext libtool gcc libcap-devel systemd-devel yajl-devel glibc-static libseccomp-devel json-c-devel},
    "debian"=> %w{make git gcc build-essential pkgconf libtool libsystemd-dev libprotobuf-c-dev libcap-dev libseccomp-dev libyajl-dev go-md2man autoconf python3 automake}
  ),
}

default["podman"]["sources"]["conmon"] = {
  url: "https://github.com/containers/conmon.git",
  tag: "v2.2.1",
  deps:  value_for_platform_family(
    %w{fedora rhel} => %w{gcc git glib2-devel glibc-devel libseccomp-devel make pkgconfig runc},
    "debian"=> %w{gcc git libc6-dev libglib2.0-dev libseccomp-dev pkg-config make runc}
  ),
}

default["podman"]["sources"]["catatonit"] = {
  url: "https://github.com/openSUSE/catatonit.git",
  tag: "v0.2.1",
  deps:  value_for_platform_family(
    %w{fedora rhel} => [],
    "debian"=> [],
  ),
}
default["podman"]["cni-plugins"]["version"] = "v1.9.1"
default["podman"]["cni-plugins"]["url"]="https://github.com/containernetworking/plugins/releases/download"
