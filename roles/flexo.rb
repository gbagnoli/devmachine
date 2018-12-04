name 'flexo'
description 'configure flexo'
run_list [
  'role[server]',
  'role[container]',
  'recipe[flexo::default]'
]
