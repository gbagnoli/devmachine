name 'ubikxps'
description 'ubik xps laptop'
run_list [
  'role[devlaptop]',
  'recipe[user::ubikxps]'
]

default_attributes(
  'user' => {
    'uid' => 1001,
    'group' => 1001
  },
  'users' => {
    'irene' => {
      'uid' => 1000,
      'group' => 1000
    }
  }
)
