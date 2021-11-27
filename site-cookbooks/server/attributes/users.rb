# either set this three
# default['server']['users'][<user>]['id'] = 200x
# default['server']['users'][<user>]['shell'] = '/bin/bash'
# default['server']['users'][<user>]['ssh_keys'] = []

# or skip management (if defined elsewhere)
# default['server']['users'][<user>]['unmanaged'] = true

# giacomo is handled by cookook 'user'
default["server"]["users"]["giacomo"]["unmanaged"] = true
default["server"]["users"]["giacomo"]["sysadmins"] = true
default["server"]["users"]["giacomo"]["delete"] = false

# dario
default["server"]["users"]["dario"]["unmanaged"] = true
default["server"]["users"]["dario"]["delete"] = false
default["server"]["users"]["dario"]["id"] = 2002
default["server"]["users"]["dario"]["shell"] = "/bin/bash"
default["server"]["users"]["dario"]["ssh_keys"] = [
  "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILDS/c+bNRBdYvNvcTSf/ptLWBtOBzf26vc/xQuPhpRC",
]
