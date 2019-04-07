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
        'enabled' => true
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
  },
  'syncthing' => {
    'users' => {
      'dario' => {
        'hostname' => 'syncthing.bloodsea.tigc.eu',
        'port' => 8384
      }
    }
  }
)
