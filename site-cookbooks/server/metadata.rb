# frozen_string_literal: true

name 'server'
maintainer 'Giacomo Bagnoli'
maintainer_email 'gbagnoli@gmail.com'
license 'MIT'
description 'Server common code'
long_description 'Server common code'
version '0.1.0'

issues_url 'https://github.com/gbagnoli/devmachine/issues'
source_url 'https://github.com/gbagnoli/devmachine'
chef_version '>=13'
supports 'ubuntu', '>= 18.04'

depends 'apt'
depends 'chef_client_updater'
depends 'hardening'
depends 'user'
depends 'syncthing'
depends 'logrotate'
