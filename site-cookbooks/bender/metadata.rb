# frozen_string_literal: true

name 'bender'
maintainer 'Giacomo Bagnoli'
maintainer_email 'gbagnoli@gmail.com'
license 'MIT'
description 'Installs/Configures bender'
long_description 'Installs/Configures bender'
version '0.1.0'

issues_url 'https://github.com/gbagnoli/devmachine/issues'
source_url 'https://github.com/gbagnoli/devmachine'
chef_version '>=13'
supports 'ubuntu', '>= 18.04'

depends 'lxd'
depends 'apt'
depends 'sysctl'
depends 'sudo'
depends 'dnscrypt_proxy'
depends 'hostsfile'
depends 'nginx-hardening'
depends 'nginx'
depends 'acme'
