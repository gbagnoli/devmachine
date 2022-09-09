# frozen_string_literal: true

name "upik"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures ubik"
version "0.1.0"

depends "chef_client_updater"
depends "btrbk"
depends "dnscrypt_proxy"
issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 18.04"
supports "debian", ">= 8.9"
