name 'bender'
description 'configure bender'
default_attributes(
  'ssh-hardening' => {
    'ssh' => {
      'server' => {
        'allow_root_with_key' => true
      }
    }
  },
  'os-hardening' => {
    'network' => {
      'forwarding' => true,
      'ipv6' => {
        'enable' => true
      }
    }
  }
)
run_list [
  'role[server]',
  'recipe[bender::default]'
]
