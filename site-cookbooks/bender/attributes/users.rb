# either set this three
# default['bender']['users'][<user>]['id'] = 1000
# default['bender']['users'][<user>]['shell'] = '/bin/bash'
# default['bender']['users'][<user>]['ssh_keys'] = []

# or skip management (if defined elsewhere

default['bender']['users']['giacomo']['manage'] = false
