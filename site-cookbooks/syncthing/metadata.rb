# frozen_string_literal: true

name "syncthing"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs and configure syncthing"
version "0.1.0"
issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 18.04"
supports "debian", ">= 8.9"
depends "podman"
