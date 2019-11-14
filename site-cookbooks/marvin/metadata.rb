# frozen_string_literal: true

name 'marvin'
maintainer 'Giacomo Bagnoli'
maintainer_email 'gbagnoli@gmail.com'
license 'MIT'
description 'Installs/Configures marvin'
long_description 'Installs/Configures marvin'
version '0.1.0'

issues_url 'https://github.com/gbagnoli/devmachine/issues'
source_url 'https://github.com/gbagnoli/devmachine'
chef_version '>=14'
supports 'ubuntu', '>= 18.04'

depends 'dnscrypt_proxy'
depends 'logrotate'
depends 'nginx', '<10'
depends 'nodejs'
depends 'openvpn'
depends 'rclone'
depends 'server'
