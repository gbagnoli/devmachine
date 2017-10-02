name 'devlaptop'
description 'A role to configure a development workstation'
run_list [
  'recipe[ubik::ppa]',
  'recipe[ubik::users]',
  'recipe[pyenv::user]',
  'recipe[ruby_build]',
  'recipe[ruby_rbenv::user]',
  'recipe[java]',
  'recipe[ubik]',
  'recipe[syncthing]'
]
default_attributes(
  'ubik' => {
    'golang' => {
      'version' => '1.9'
    },
    'languages' => %w(en it),
    'enable_mtrack' => false,
    'install_latex' => true
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
    'git_ref' => 'v1.1.4',
    'user_installs' => [{
      'user' => 'giacomo',
      'pythons' => ['2.7.14', '3.6.2'],
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
    'git_ref' => 'v1.1.1',
    'update' => true,
    'user_installs' => [{
      'user' => 'giacomo',
      'rubies' => ['2.4.2'],
      'global' => '2.4.2',
      'gems' => {
        '2.4.2' => [
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
  }
)
