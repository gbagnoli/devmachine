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
chef_version '>=12'
supports 'ubuntu', '>= 16.04'
supports 'debian', '>= 8.9'

depends 'oauth2_proxy'
depends 'nginx'
depends 'nodejs'
