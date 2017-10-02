name 'ubiktest'
description 'ubik test'
run_list [
  'role[devlaptop]'
]

default_attributes(
  'user' => {
    'uid' => 1001,
    'gid' => 1001,
    'install_vpnutils' => false
  },
  'ubik' => {
    'enable_mtrack' => true,
    'install_latex' => false
  },
  'syncthing' => {
    'skip_service' => true
  },
  'users' => {
    'irene' => {
      'uid' => 1002,
      'gid' => 1002
    }
  }
)
