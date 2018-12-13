name 'bloodsea'
description 'configure bloodsea'
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
      'dario' => {
        'unmanaged' => false
      }
    }
  }
)
