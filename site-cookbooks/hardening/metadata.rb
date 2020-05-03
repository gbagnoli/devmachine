# frozen_string_literal: true

name "hardening"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Wrapper for the various hardening cookbooks"
long_description "Wrapper for the various hardening cookbooks"
version "0.1.0"

depends "os-hardening"
depends "ssh-hardening"
issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 18.04"
supports "debian", ">= 8.9"
