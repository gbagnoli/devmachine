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
  pubkey: 'AAAAB3NzaC1yc2EAAAADAQABAAACAQCmr/XiQz6S+c/gUo0ihuyTRTiMJ2w+ma3EXaiarjGXtwZbzUxHxIK52xU+bhVeKiqX/V0yFGTcWdyn+biBUVx6bTPr0vTbx+Td4+HAIDcUdGRp90FWdXV71BIZQc2HDKz+15wtMoNqhiPAo7bw3knTB6ZhEtiEbcYW5rNrQJiX729RLct3V5KIlThwu3BASQSm3hLSQeajEUq5x25OhvOuyAeqkRhuXApcHoj8qFrW0zl+d0L7rVzXVSI2+p4Q/rJmP44KPeDaKXaSTF3F/7vfQ8vMREvEZKSTzeXTDu/m+2T54sm3rmFl+qZ/8WIxCqOjy2DIbZ1911YZD2fYZ2Plfs85y5KMbXe8frgF4RFqpL2+3RszmytIU2iikoXulkWpG9pXJTHqLQZOythvpSdQ6sV0vHtAKc8J0QFGE/Q9Q1PgRBOZIaqRtscuWiJxn0kAHxvL3V2aLmejh+OyNs3jehw4TcIMilvl5YDfCHeDBzrEG/1+DUkMcOfOr6BA6ZUAwVIoApcaNJn4qN0c3NmxBBQ5UtpYADn7Aw8kAh9aVkowHd4Z38Rug+ZealFR4ybXtX2u5y7EKAHs7+fG5JbAekoMZ1bg1QkkkujvBb4ZpsL5tSMn80MTbDVs/HeSBfq+sP5UvByk3Yrm8SsCU2RVIvovB0kIMfEOAiJ2tU3nPQ==' # rubocop:disable Metrics/LineLength
}, {
  name: 'jdoe@oldpc',
  keytype: 'ssh-ed25519',
  pubkey: 'AAAAC3NzaC1lZDI1NTE5AAAAIJ7IqCcv/ybJQ8Y0u60y3JltCDqgQ+UZyTzjVwiYY1m+'
}]
