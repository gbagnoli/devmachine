default['lxd']['config_dir'] = '/etc/lxd'
# lxd configuration will be preseeded via YAML loading
# please read the doc to understand implications
# https://lxd.readthedocs.io/en/latest/preseed/
# quick, and real dirty.
default['lxd']['config'] = <<~HEREDOC
  config: {}
  networks:
    - name: lxdbr0



HEREDOC
