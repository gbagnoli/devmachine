# frozen_string_literal: true

name "beelzebot"
maintainer "Giacomo Bagnoli"
maintainer_email "gbagnoli@gmail.com"
license "MIT"
description "Installs/Configures beelzebot"
long_description "Installs/Configures beelzebot"
version "0.1.0"

issues_url "https://github.com/gbagnoli/devmachine/issues"
source_url "https://github.com/gbagnoli/devmachine"
chef_version ">=14"
supports "ubuntu", ">= 18.04"

depends "git"
depends "logrotate"
depends "nodejs"
depends "server"
