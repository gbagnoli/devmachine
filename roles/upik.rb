name 'upik'
description 'Configure upik'
run_list [
  'recipe[omnibus_updater]',
  'recipe[btrbk]',
  'recipe[upik::mounts]',
  'recipe[upik::default]',
  'recipe[syncthing]',
  'recipe[dnscrypt_proxy]'
]

default_attributes(
  'syncthing' => {
    'users' => {
      'up' => nil
    }
  }
)
