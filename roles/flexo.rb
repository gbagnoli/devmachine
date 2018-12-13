name 'flexo'
description 'configure flexo'
run_list [
  'role[server]',
  'role[container]',
  'recipe[flexo::default]'
]

default_attributes(
  'server' => {
    'users' => {
      'fnigi' => {
        'unmanaged' => false
      },
      'dario' => {
        'unmanaged' => false
      },
      'sonne' => {
        'unmanaged' => false
      }
    }
  }
)
