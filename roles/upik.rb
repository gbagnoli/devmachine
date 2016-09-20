name 'upik'
description 'Configure upik'
run_list [
  'recipe[omnibus_updater]',
  'recipe[btrbk]',
  'recipe[upik::mounts]',
  'recipe[upik::default]',
  'recipe[user]',
  'recipe[syncthing]',
  'recipe[dnscrypt_proxy]'
]

default_attributes(
  'syncthing' => {
    'users' => {
      'up' => nil
    }
  },
  'user' => {
    'login' => 'up',
    'group' => 'up',
    'uid' => 1000,
    'gid' => 1000,
    'realname' => 'ubik'
  }
)
