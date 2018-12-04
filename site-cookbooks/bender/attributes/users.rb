# either set this three
# default['bender']['users'][<user>]['id'] = 200x
# default['bender']['users'][<user>]['shell'] = '/bin/bash'
# default['bender']['users'][<user>]['ssh_keys'] = []

# or skip management (if defined elsewhere)
# default['bender']['users'][<user>]['unmanaged'] = true

# giacomo
default['bender']['users']['giacomo']['unmanaged'] = true

# rubocop:disable LineLength
# fnigi
default['bender']['users']['fnigi']['id'] = 1001
default['bender']['users']['fnigi']['shell'] = '/bin/bash'
default['bender']['users']['fnigi']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQDiNEgGDeGozxr0UAV+TbDgoC5Lgemfa+uCDwjTriF3VebPFhNxXbzYh9NXKnj7X294XUIwwVBSJ7GOy6HavI5pg8Kmr/8K3/tkvpP9ojqSukLHuULHL4Ks3vH9ZKRvWockZCV3FZ2TUpM1cwrIlnX2r/bjhidXv+jhutO9ASi73YkfDmPIiSXyL2GnS0VoRemYV/3O3cdK+W2Y8ycluecSOhlpEK03AJmdPfBgSr+sww/OO6ofSk+bxACZS/uWcJR+4rfB1nTRp7uyabJgVIdzQ3t3/3MmSQdKnlU8Q5G3Q7KN/ANA+QzqoER1oYP0kfYNzHGKOOoWwzauwq6jMCxtX+0DuyQzECbczbvZgtbxu8/8EX9KN0ay91/4jyNatfXrlzU/2YuycNDHp2LKM70k/gkSog69pbhpEbeLUp14VvGbRKXMBil2h6swZCiW55eLQ77ZIjUasEBTOHeKWQWg6QFQYKqLMuis0gxsgViPd0qQWjbNKNgVip/pi97Xv9OPpAx4HZVoblYVqIRGF28QaosIiTHk0tv1XdJvS41fDtAWRw+cNvz64EYWVRVSVci7q0Nati/qyaKhYtZXW0SoBsYnQlp+0bbnKRSQviZRwF/xMwN2D1F1h/qLd1q7awuMSMUR7OFmOEiJtyLgB8pcPZVk+Xo8TwhA2xhSxHY/cQ==',
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQCx5Ue76VVttCI1KBLLmnpAaKdrbe/1N7rf2KOjhB+mKIp3DD0CdSgpTXDTEitz9PZCKhE1RCWXpzWFXW+IxtmKcEKvBWMWnTFphe8lMDpH8gIgMjeVHzxmz3JD4gae8QQWh6pzIxOtVakGAPHAWXClo4S7JE5QmPVjcUjMUBfWDii+Tu815sZxxXhfznnea4QbDRlCPJ/HQsnYtcLWPuVc0OnVEWMdSY4m+rgZbyr37QKsaTyfTdvwQikUaAG75ZhE9n42aRmPrAvh4cjEjT7+lbxk/QTjDItyNV8ZNUtExaiLRAZVQKAQrRx5Hx0CCkTtRHMN09QJ4rsCRuD9gLXb'
]

# raistlin
default['bender']['users']['raistlin']['id'] = 1002
default['bender']['users']['raistlin']['shell'] = '/bin/bash'
default['bender']['users']['raistlin']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQCuY8Aa+fb54bZWXwvlZFbY8Iv0jUIMxfOP905QFb7+BSxnfrQGTL1oEMNGD1U0p1aRCQPTg+MqeKKMMDt5RqOhs9JNiZC4nfwY65BFIay7jpu1URmAYL1Um3wE6+WTxhJymIfA5nU5vZByQM1Q4uSjtXYRGkKjG3o6Ei8j/H73TDMd+gkOLcSu58s+VBz2CxvUw9Bf9ZvJ0Q38aywv9oPVVAbUp+VLuQxV7BjfUDLDhPducQkwTh0TfL2K4/cLCXZmrzWmU0gx5bSwfYDicutXiKxh+1jAgUibzkLEJy9UIoz1rJqsQjrmdUPKryffO6BxnXMMUtw3vJ/vl4NEZdtj'
]

# sonne
default['bender']['users']['sonne']['id'] = 1003
default['bender']['users']['sonne']['shell'] = '/bin/bash'
default['bender']['users']['sonne']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQDKIoSFOFogiASt6IUBirk210BhZScRljRvGfmHDjeTon9/uh2ENL2zPfbxoE6ws4nlqOBL1DDNZ4pWHF5FmY6Hzi5E+z8a1pS5Jhwt7vXJwSCuy8EnD0i608bwzcSpskbgI/rHeuftB9vhUCQYZ1SzeTJYTdjyweyHhP99aWRse24u12upQn6uSGuH1LnCbijiKvqFR9KsJw3vg9pDGtdvH7EPdGvJeTYwlmhZt9O4K75NRzy09iCJDZpayz+hIVm0V7T/n4HMGyzno6FBTYJCc1DZn/qVQglFQ5KdERPFWKqjW8e9j1CQjcxQyRJtS447Wjy8Lp1yxEar5NdSw58XieQGRQQMSGR9oDlwt5OPjglQU3CqVV7Rxryh+iSuMcL6NphGlXtzzyQWUpRCWSgicPK58mRrVJ2CeKWw2VvHJn6UdtTtZI10ZwFYzgKZlNYtmqvZROFRf0net9ImfaaUehDCf/Gg3bq515j1yoiUTl7PYjQUX6ty9s6nhWL2Z6jog3daU3wv9IJcp6+y+G67kqVfJ9Et+ltq2DLPPY1ppFhvzqjJcsrnRq0Ib5134C0qKI9S3mJoN3sFRgfB2jgFzx6B6soc5LqcLzdOnvy3EhZNRjfDIky5yLnQIyiyE8LbUMOcQWAnszSn2XGbb9h/JLXotP703aLwLAh2eTzSFw=='
]
# rubocop:enable LineLength
