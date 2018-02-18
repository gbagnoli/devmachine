name 'marvin'
description 'configure marvin'
run_list [
  'role[server]',
  'recipe[openvpn]',
  'recipe[marvin::default]'
]
