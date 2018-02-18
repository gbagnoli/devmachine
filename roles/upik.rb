name 'upik'
description 'Configure upik'
run_list [
  'recipe[apt::unattended-upgrades]',
  'recipe[chef_client_updater]',
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

override_attributes(
  'apt' => {
    'unattended_upgrades' => {
      'enable' => true,
      'mail' => 'gbagnoli@gmail.com',
      'allowed_origins' => [
        'Debian:stable',
        'Debian:stable-updates',
        'Syncthing:syncthing',
        'ubilinux:ubilinux3-upboard',
        '. wheezy:wheezy'
      ],
      'remove_unused_dependencies' => true
    }
  },
  'chef_client_updater' => {
    'version' => '13',
    'upgrade_delay' => 0,
  },
  'upik' => {
    'skip_mounts' => false
  }
)
