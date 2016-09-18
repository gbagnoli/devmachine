user = 'giacomo'
group = 'giacomo'
home = "/home/#{user}"
uid = 1000
gid = 1000
realname = 'Giacomo Bagnoli'

user user do
  group group
  shell '/bin/bash'
  uid uid
  gid gid
  home home
  comment realname
end
