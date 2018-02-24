name 'server'
description 'base config for server'
run_list [
  'recipe[apt]',
  'recipe[hardening]',
  'recipe[chef_client_updater]',
  'recipe[user]',
  'recipe[syncthing]',
  'recipe[apt::unattended-upgrades]'
]

default_attributes(
  'apt': {
    'unattended_upgrades': {
      'enable': true,
      'mail': 'gbagnoli@gmail.com',
      'remove_unused_dependencies': true,
      'allowed_origins': [
        '${distro_id}:${distro_codename}',
        '${distro_id}:${distro_codename}-security',
        '${distro_id}:${distro_codename}-updates',
        '${distro_id}:${distro_codename}-proposed',
        '${distro_id}:${distro_codename}-backports'
      ]
    }
  },
  'chef_client_updater': {
    'version': '13',
    'upgrade_delay': 0
  },
  'os-hardening': {
    'network': {
      'ipv6': {
        'enable': true
      }
    },
    'security': {
      'kernel': {
        'enable_module_loading': true,
        'disable_filesystems': %w[cramfs freevxfs jffs2 hfs
                                  hfsplus squashfs udf]
      }
    }
  },
  'ssh-hardening': {
    'ssh': {
      'server': {
        'sftp': {
          'enable': true
        }
      }
    }
  }
)
