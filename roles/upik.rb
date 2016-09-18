name 'upik'
description 'Configure upik'
run_list [
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
