# frozen_string_literal: true

name "user"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures user"
version "0.1.0"

depends "git"
depends "gnome"
depends "ssh_known_hosts"
depends "sudo"
depends "ssh_authorized_keys"
issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 18.04"
supports "debian", ">= 8.9"
