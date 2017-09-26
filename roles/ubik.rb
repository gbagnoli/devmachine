name 'ubik'
description 'ubik mac laptop'
run_list [
  'role[devlaptop]'
]

default_attributes(
  'user' => {
    'uid' => 1000,
    'group' => 1000
  },
  'ubik' => {
    'enable_mtrack' => true
  }
)
