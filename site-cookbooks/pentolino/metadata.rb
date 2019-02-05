# frozen_string_literal: true

name 'pentolino'
maintainer 'Fabio Nigi'
maintainer_email 'fabio@nigis.eu'
license 'MIT'
description 'Installs/Configures pentolino'
long_description 'Installs/Configures pentolino'
version '0.1.0'

issues_url 'https://github.com/gbagnoli/devmachine/issues'
source_url 'https://github.com/gbagnoli/devmachine'
chef_version '>=14'
supports 'ubuntu', '>= 18.04'

depends 'dnscrypt_proxy'
depends 'logrotate'
depends 'openvpn'
