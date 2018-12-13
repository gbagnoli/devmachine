name 'pentolino'
description 'configure pentolino'
run_list [
  'role[server]',
  'role[container]'
]

default_attributes(
  'server' => {
    'components' => {
      'syncthing' => {
        'enabled' => false
      },
      'user' => {
        'enabled' => false
      }
    },
    'users' => {
      'fnigi' => {
        'unmanaged' => false
      }
    }
  }
)
