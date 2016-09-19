name 'upik'
description 'Configure upik'
run_list [
  'recipe[omnibus_updater]',
  'recipe[upik::default]',
  'recipe[upik::mounts]',
  'recipe[syncthing]'
]

default_attributes(
  'syncthing' => {
    'users' => {
      'up' => nil
    }
  }
)
