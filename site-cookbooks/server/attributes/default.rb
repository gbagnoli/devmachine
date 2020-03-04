default['server']['components']['syncthing']['enabled'] = true
default['server']['components']['user']['enabled'] = true
default['server']['components']['chef_client_cron']['enabled'] = true

default['server']['chef']['cron']['hour'] = '*'
default['server']['chef']['cron']['minute'] = '30'

default['chef_client']['config'] = {}
