name 'ubik'
description 'A role to configure a development workstation'
run_list [
  'recipe[ubik::ppa]',
  'recipe[pyenv::user]',
  'recipe[ruby_build]',
  'recipe[ruby_rbenv::user]',
  'recipe[java]',
  'recipe[ubik]',
  'recipe[syncthing]',
  'recipe[dnscrypt_proxy]',
  'recipe[profile_sync_daemon]'
]
default_attributes(
  'ubik' => {
    'golang' => {
      'version' => '1.7.1'
    },
    'languages' => %w(en it),
    'enable_mtrack' => true
  },
  'syncthing' => {
    'users' => {
      'giacomo' => nil,
      'irene' => {
        'hostname' => 'ubik-irene',
        'port' => 8385
      }
    }
  },
  'pyenv' => {
    'git_ref' => 'v1.0.1',
    'user_installs' => [{
      'user' => 'giacomo',
      'pythons' => ['3.5.2'],
      'global' => 'system'
    }]
  },
  'java' => {
    'install_flavor' => 'oracle',
    'jdk_version' => '8',
    'oracle' => {
      'accept_oracle_download_terms' => true
    }
  },
  'rbenv' => {
    'git_ref' => 'v1.0.0',
    'update' => true,
    'user_installs' => [{
      'user' => 'giacomo',
      'rubies' => ['2.3.1'],
      'global' => '2.3.1',
      'gems' => {
        '2.3.1' => [
          { 'name' => 'bundler' },
          { 'name' => 'rubocop' }
        ]
      }
    }]
  },
  'user' => {
    'login' => 'giacomo',
    'group' => 'giacomo',
    'uid' => 1000,
    'gid' => 1000,
    'realname' => 'Giacomo Bagnoli',
    'install_vpnutils' => true
  },
  'profile_sync_daemon' => {
    'users' => %w(giacomo irene)
  }
)
