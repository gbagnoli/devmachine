# lxd-storage-$poolname-attrs
# see for drivers / attrs https://lxd.readthedocs.io/en/latest/storage/
# attrs at https://github.com/lxc/lxd/blob/master/doc/storage.md
default['lxd']['storage']['pools']['default']['driver'] = 'btrfs'
default['lxd']['storage']['pools']['default']['attrs'] = nil
