name 'devmachine'
description 'A role to configure a development workstation'
run_list [
  'recipe[ubik::ppa]',
  'recipe[pyenv::user]',
  'recipe[java]',
  'recipe[ubik]'
]
default_attributes(
  'pyenv' => {
    'git_ref' => 'v1.0.1',
    'user_installs' => [{
      'user' => 'giacomo',
      'pythons' => ['3.5.2', '2.7.8'],
      'global' => 'system'
    }]
  },
  'java' => {
    'install_flavor' => 'oracle',
    'jdk_version' => '8',
    'oracle' => {
      'accept_oracle_download_terms' => true
    }
  }
)
