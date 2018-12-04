name 'marvin'
description 'configure marvin'
run_list [
  'role[server]',
  'recipe[marvin::default]'
]

default_attributes(
  'ssh-hardening' => {
    'ssh' => {
      'server' => {
        'allow_root_with_key' => true
      }
    }
  },
  'os-hardening' => {
    'components' => {
      'auditd' => false
    }
  },
  'user' => {
    'uid' => '4000',
    'gid' => '4000'
  }
)
