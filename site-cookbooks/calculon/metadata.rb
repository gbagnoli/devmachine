# frozen_string_literal: true

name "calculon"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures calculon"
version "0.1.0"

issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=17"
supports "fedora"
supports "rocky", ">= 9.0"

depends "hostsfile"
depends "datadog"
depends "pihole"
depends "podman"
depends "podman_nginx"
depends "yum-epel"
depends "yum-elrepo"
depends "logrotate"
depends "btrbk"
depends "tailscale"
depends "syncthing"
gem 'toml'
