# frozen_string_literal: true

name 'flexo'
maintainer 'Giacomo Bagnoli'
maintainer_email 'gbagnoli@gmail.com'
license 'MIT'
description 'Installs/Configures flexo'
long_description 'Installs/Configures flexo'
version '0.1.0'

issues_url 'https://github.com/gbagnoli/devmachine/issues'
source_url 'https://github.com/gbagnoli/devmachine'
chef_version '>=14'
supports 'ubuntu', '>= 18.04'

depends 'datadog'
depends 'git'
depends 'logrotate'
depends 'nginx'
depends 'nodejs'
depends 'plex'
depends 'poise-python'
depends 'server'
