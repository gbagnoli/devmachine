name 'ubikxps'
description 'ubik xps laptop'
run_list [
  'role[devlaptop]'
]

default_attributes(
  'user' => {
    'uid' => 1001,
    'group' => 1001
  }
)
