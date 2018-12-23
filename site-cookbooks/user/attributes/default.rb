# frozen_string_literal: true

default['user']['login'] = 'giacomo'
default['user']['group'] = 'giacomo'
default['user']['uid'] = 1000
default['user']['gid'] = 1000
default['user']['realname'] = 'Giacomo Bagnoli'
default['user']['install_vpnutils'] = false
default['user']['ssh_authorized_keys'] = [{
  name: 'giacomo@ubikxps',
  keytype: 'ssh-ed25519',
  pubkey: 'AAAAC3NzaC1lZDI1NTE5AAAAICYS/jhr4/Ld55BT2YjP+b+LHWNkaSuRYSuLre2Mbxwz'
}, {
  name: 'giacomo@ubik',
  pubkey: 'AAAAC3NzaC1lZDI1NTE5AAAAIInHiA0bvhG12svBVSkAJ8Aug/9u5kFYR9jVWnaxd5/6',
  keytype: 'ssh-ed25519'
}, {
  name: 'giacomo@giacomo-mbp',
  pubkey: 'AAAAC3NzaC1lZDI1NTE5AAAAIOUhSAil47ZMSFaYq0xFD1ctCJlszWhdevIBqQKmaBsv',
  keytype: 'ssh-ed25519'
}, {
  name: 'giacomo@android',
  keytype: 'ssh-rsa',
  pubkey: 'AAAAB3NzaC1yc2EAAAABIwAAAgEAtVrcXBTJG5U/CfpgO3C6DXsgQSJZpRAaNvfa5kq+/z/Jog7boatVUPpWmq0IUzO6cdHhQ04bNBlb5ycPWxACQC0tXRmdpUBW3CXyLMn595s1W9zJGV7hdZDX9t2EWox4ZshKsxu31PZ6RArBLTjki8EGwh66TdG+PPcrTwtZA32rjdOm12K560ySH8keSKSX1I298ITe/9AOzWqhl4cN/MywiDuIq8/ZozRSbffc5hOzW8JpRLHtccnAF15kvuyffHnUpBkES0sKHCBdXIp83uha/ahNPbTri6pmFJ/Vsc+UZkhINYK+63jWxNafCgWrMi8acK5qiU+Q+C3ujyHL8Lk4kjVrM6nqSDfc33OXOGkH5Z7QQFm1jOxbUuuM7oKNiIWd2uGiTRcvQVZto5rlaSPAfiM2O13bHKRMOj//wYsDeF1XcZPU42X+g/HIqbEy8r2sBGPxlX0n/z89lgBfXDxNIxm4Nhj7LEhV6OpfU6RCuK2K79tbQ7Ajh0QHzkiRQuLlet3KeeGJzs9LBC2fHpR+0d8ZATIwXB4RwRyprpUmln8sjmLzrn18lH/n+/zZ+GLiLPC1EYtJRR8DaT4gW/7fJxVZtW3hzhnkzwvPOL9YVgS17RfxweZE4ftXZAzrnPQZ0OZ/fxnuRey+BDp5BFIiMBAOFo2uzvHVnsD/Jj8=' # rubocop:disable Metrics/LineLength
}, {
  name: 'jdoe@oldpc',
  keytype: 'ssh-ed25519',
  pubkey: 'AAAAC3NzaC1lZDI1NTE5AAAAIJ7IqCcv/ybJQ8Y0u60y3JltCDqgQ+UZyTzjVwiYY1m+'
}]
