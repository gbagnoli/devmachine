# lxd-storage-$poolname-attrs
# see for drivers / attrs https://lxd.readthedocs.io/en/latest/storage/
# attrs at https://github.com/lxc/lxd/blob/master/doc/storage.md
default['lxd']['storage']['pool']['default']['driver'] = 'btrfs'
default['lxd']['storage']['pool']['default']['attrs'] = nil
