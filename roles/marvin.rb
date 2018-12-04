name 'marvin'
description 'configure marvin'
run_list [
  'role[server]',
  'role[container]',
  'recipe[marvin::default]'
]
