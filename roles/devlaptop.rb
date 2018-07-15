name 'devlaptop'
description 'A role to configure a development workstation'
run_list [
  'recipe[ubik::ppa]',
  'recipe[hardening]',
  'recipe[ubik::users]',
  'recipe[ubik::python]',
  'recipe[ubik::ruby]',
  'recipe[java]',
  'recipe[openvpn]',
  'recipe[ubik]',
  'recipe[syncthing]'
]
default_attributes(
  'authorization' => {
    'sudo' => {
      'include_sudoers_d' => true
    }
  },
  'ubik' => {
    'golang' => {
      'version' => '1.10'
    },
    'languages' => %w[en it],
    'enable_mtrack' => false,
    'install_latex' => true,
    'install_fonts' => true
  },
  'os-hardening' => {
    'auth' => {
      'retries' => 15,
      'lockout_retries' => 300,
      'timeout' => 120
    },
    'desktop' => {
      'enable' => true
    },
    'network' => {
      'ipv6' => {
        'enable' => true
      }
    },
    'security' => {
      'kernel' => {
        'enable_module_loading' => true,
        'disable_filesystems' => %w[cramfs freevxfs jffs2 hfs
                                    hfsplus squashfs udf]
      }
    }
  },
  'syncthing' => {
    'users' => {
      'giacomo' => nil,
      'irene' => {
        'hostname' => 'ubik-irene',
        'port' => 8385
      }
    }
  },
  'pyenv' => {
    'git_ref' => 'v1.2.5',
    'user_installs' => [{
      'user' => 'giacomo',
      'pythons' => ['2.7.14', '3.7.0'],
      'global' => 'system',
      'upgrade' => 'sync'
    }]
  },
  'java' => {
    'install_flavor' => 'oracle',
    'jdk_version' => '8',
    'oracle' => {
      'accept_oracle_download_terms' => true
    }
  },
  'ruby_build' => {
    'upgrade' => 'sync'
  },
  'rbenv' => {
    'git_ref' => 'v1.1.1',
    'user_installs' => [{
      'upgrade' => 'sync',
      'user' => 'giacomo',
      'plugins' => [{
        'name' => 'chefdk',
        'git_url' => 'https://github.com/docwhat/rbenv-chefdk.git'
      }],
      'rubies' => ['2.4.4', '2.5.1'],
      'global' => '2.4.4',
      'gems' => {
        '2.4.4' => [
          { 'name' => 'bundler' },
          { 'name' => 'rubocop' }
        ],
        '2.5.1' => [
          { 'name' => 'bundler' },
          { 'name' => 'rubocop' }
        ]
      }
    }]
  },
  'user' => {
    'login' => 'giacomo',
    'group' => 'giacomo',
    'uid' => 1000,
    'gid' => 1000,
    'realname' => 'Giacomo Bagnoli',
    'install_vpnutils' => true
  }
)
