name 'marvintest'
description 'test marvin'
run_list [
  'role[marvin]'
]

override_attributes(
  'user' => {
    'uid' => 1500,
    'gid' => 1500
  }
)
