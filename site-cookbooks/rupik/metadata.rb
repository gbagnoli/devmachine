# frozen_string_literal: true

name "rupik"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures rubik"
version "0.1.0"

depends "btrbk"
depends "argonone"
depends "podman"
depends "syncthing"
depends "tailscale"
issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 20.04"
