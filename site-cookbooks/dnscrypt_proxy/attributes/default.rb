# frozen_string_literal: true

# old directory of the autoinstall script, now used for cleanup
default['dnscrypt_proxy']['autoinstall_src_dir'] = '/usr/local/src/dnscrypt-autoinstall'

default['dnscrypt_proxy']['version'] = '2.0.19'
default['dnscrypt_proxy']['repository_url'] = 'https://github.com/jedisct1/dnscrypt-proxy/'
default['dnscrypt_proxy']['arch'] = 'linux_x86_64'

default['dnscrypt_proxy']['listen_address'] = '0.0.0.0'
default['dnscrypt_proxy']['listen_port'] = 53
