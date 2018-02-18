name 'server'
description 'base config for server'
run_list [
  'recipe[apt::unattended-upgrades]',
  'recipe[chef_client_updater]'
]

default_attributes(
  'apt' => {
    'unattended_upgrades' => {
      'enable' => true,
      'mail' => 'gbagnoli@gmail.com',
      'remove_unused_dependencies' => true
    }
  },
  'chef_client_updater' => {
    'version' => '13',
    'upgrade_delay' => 0
  }
)
