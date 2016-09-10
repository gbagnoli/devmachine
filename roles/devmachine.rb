name 'devmachine'
description 'A role to configure a development workstation'
run_list [
  'recipe[ubik::ppa]',
  'recipe[pyenv::user]',
  'recipe[ruby_build]',
  'recipe[ruby_rbenv::user]',
  'recipe[java]',
  'recipe[ubik]'
]
default_attributes(
  'ubik' => {
    'golang' => {
      'version' => '1.7.1'
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
  }
)
