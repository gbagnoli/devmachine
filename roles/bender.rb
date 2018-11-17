name 'bender'
description 'configure bender'
run_list [
  'role[server]',
  'recipe[bender::default]'
]
