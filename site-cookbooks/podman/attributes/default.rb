# rubocop:disable Layout/LineLength
default["podman"]["go"]["version"] = "1.22.3"
default["podman"]["rocky"]["podman"]["git"] = {
  url: "https://github.com/containers/podman.git",
  tag: "v5.0.1",
  # btrfs-devel is already installed from downloaded rpm in rocky
  deps:  %w{go systemd-devel gpgme-devel libseccomp-devel ostree-devel shadow-utils-subid-devel},
  download: [{
    url: "https://kojipkgs.fedoraproject.org//packages/containers-common/1/95.fc39/noarch",
    rpms: %w{containers-common-1-95.fc39.noarch.rpm containers-common-extra-1-95.fc39.noarch.rpm}
  }]
}

default["podman"]["rocky"]["crun"]["git"] = {
  url: "https://github.com/containers/crun.git",
  tag: "1.14.4",
  deps: %w{make automake autoconf gettext libtool gcc libcap-devel systemd-devel yajl-devel glibc-static libseccomp-devel},
}

default["catatonic"]["git"] = "https://github.com/openSUSE/catatonit.git"
# rubocop:enable Layout/LineLength
