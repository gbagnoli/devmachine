# frozen_string_literal: true

name "calculon"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures calculon"
version "0.1.0"

issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=18"
supports "fedora"

depends "hostsfile"
depends "datadog"
# depends "nginx"
# depends "acme"
