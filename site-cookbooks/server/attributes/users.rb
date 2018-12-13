# either set this three
# default['server']['users'][<user>]['id'] = 200x
# default['server']['users'][<user>]['shell'] = '/bin/bash'
# default['server']['users'][<user>]['ssh_keys'] = []

# or skip management (if defined elsewhere)
# default['server']['users'][<user>]['unmanaged'] = true

# giacomo is handled by cookook 'user'
default['server']['users']['giacomo']['unmanaged'] = true
default['server']['users']['giacomo']['sysadmins'] = true

# rubocop:disable LineLength
# fnigi
default['server']['users']['fnigi']['unmanaged'] = true
default['server']['users']['fnigi']['id'] = 2001
default['server']['users']['fnigi']['shell'] = '/bin/bash'
default['server']['users']['fnigi']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQDiNEgGDeGozxr0UAV+TbDgoC5Lgemfa+uCDwjTriF3VebPFhNxXbzYh9NXKnj7X294XUIwwVBSJ7GOy6HavI5pg8Kmr/8K3/tkvpP9ojqSukLHuULHL4Ks3vH9ZKRvWockZCV3FZ2TUpM1cwrIlnX2r/bjhidXv+jhutO9ASi73YkfDmPIiSXyL2GnS0VoRemYV/3O3cdK+W2Y8ycluecSOhlpEK03AJmdPfBgSr+sww/OO6ofSk+bxACZS/uWcJR+4rfB1nTRp7uyabJgVIdzQ3t3/3MmSQdKnlU8Q5G3Q7KN/ANA+QzqoER1oYP0kfYNzHGKOOoWwzauwq6jMCxtX+0DuyQzECbczbvZgtbxu8/8EX9KN0ay91/4jyNatfXrlzU/2YuycNDHp2LKM70k/gkSog69pbhpEbeLUp14VvGbRKXMBil2h6swZCiW55eLQ77ZIjUasEBTOHeKWQWg6QFQYKqLMuis0gxsgViPd0qQWjbNKNgVip/pi97Xv9OPpAx4HZVoblYVqIRGF28QaosIiTHk0tv1XdJvS41fDtAWRw+cNvz64EYWVRVSVci7q0Nati/qyaKhYtZXW0SoBsYnQlp+0bbnKRSQviZRwF/xMwN2D1F1h/qLd1q7awuMSMUR7OFmOEiJtyLgB8pcPZVk+Xo8TwhA2xhSxHY/cQ==',
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQCx5Ue76VVttCI1KBLLmnpAaKdrbe/1N7rf2KOjhB+mKIp3DD0CdSgpTXDTEitz9PZCKhE1RCWXpzWFXW+IxtmKcEKvBWMWnTFphe8lMDpH8gIgMjeVHzxmz3JD4gae8QQWh6pzIxOtVakGAPHAWXClo4S7JE5QmPVjcUjMUBfWDii+Tu815sZxxXhfznnea4QbDRlCPJ/HQsnYtcLWPuVc0OnVEWMdSY4m+rgZbyr37QKsaTyfTdvwQikUaAG75ZhE9n42aRmPrAvh4cjEjT7+lbxk/QTjDItyNV8ZNUtExaiLRAZVQKAQrRx5Hx0CCkTtRHMN09QJ4rsCRuD9gLXb'
]

# dario
default['server']['users']['dario']['unmanaged'] = true
default['server']['users']['dario']['id'] = 2002
default['server']['users']['dario']['shell'] = '/bin/bash'
default['server']['users']['dario']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQDKWR1plRpIJRL7KoITtE6a+xdRM6Aiq8OWX+rin7XUzoDPygA9zsajwpS7dn6iY1rRkbtdLPiakViHLnpTIwwVLYgUJseWe5MwGHfiDxTs6kr2tK/MMhvue+aBDeclUC1Le/xBlSHjY7+nyScZJrzDVv/EVE3Q4BS879tWSY6QMKHgp+d6ARHP5E4x4J9kaiD+rEBJOweFG6+FuyS2GAj/fmWbolrb+n72EgbDl9txqCRHlQMMKOCsEHj6kztb+tSZlcMOLmUixYMw9Ye1PJ0c7Rt5W6f5QgoDQLgurIiJ7V7ld17TNSzn1ifdsZnEydZaFq91yoHxCs4X+0o0hpxOr2MpSqgd8KQHNm+fLhM8z+lw0lgYS9tOlS3bFskIAOzLxXlSWZa2jFIM26G9R5/mibKBMOcUiogRSI/F8h7DRLACR4IFK0uiRzQdVHfpQ9NBJ0/ZoVXfGx6Paz8p9PkfVP1xoiKb3vgt9CADCAIBwekHdVDYCXzsedUWDkUhtpHzIY8krq8w4x4pbg8EojA9PfgustDVzo5KuDZqLPXL8qYD1IKoPLPsTFIdYbO7UVfCANdx+2AgmP9RrJUz+kBdkXzDqtESmA1IPd66TJwKjzwSRv0MLyjl1KacbpKrQDI7kg3GVSE3KzrrhCy+APcXgjUXpnH6UvyLOX66QomrVQ=='
]

# sonne
default['server']['users']['sonne']['unmanaged'] = true
default['server']['users']['sonne']['id'] = 2003
default['server']['users']['sonne']['shell'] = '/bin/bash'
default['server']['users']['sonne']['ssh_keys'] = [
  'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQDKIoSFOFogiASt6IUBirk210BhZScRljRvGfmHDjeTon9/uh2ENL2zPfbxoE6ws4nlqOBL1DDNZ4pWHF5FmY6Hzi5E+z8a1pS5Jhwt7vXJwSCuy8EnD0i608bwzcSpskbgI/rHeuftB9vhUCQYZ1SzeTJYTdjyweyHhP99aWRse24u12upQn6uSGuH1LnCbijiKvqFR9KsJw3vg9pDGtdvH7EPdGvJeTYwlmhZt9O4K75NRzy09iCJDZpayz+hIVm0V7T/n4HMGyzno6FBTYJCc1DZn/qVQglFQ5KdERPFWKqjW8e9j1CQjcxQyRJtS447Wjy8Lp1yxEar5NdSw58XieQGRQQMSGR9oDlwt5OPjglQU3CqVV7Rxryh+iSuMcL6NphGlXtzzyQWUpRCWSgicPK58mRrVJ2CeKWw2VvHJn6UdtTtZI10ZwFYzgKZlNYtmqvZROFRf0net9ImfaaUehDCf/Gg3bq515j1yoiUTl7PYjQUX6ty9s6nhWL2Z6jog3daU3wv9IJcp6+y+G67kqVfJ9Et+ltq2DLPPY1ppFhvzqjJcsrnRq0Ib5134C0qKI9S3mJoN3sFRgfB2jgFzx6B6soc5LqcLzdOnvy3EhZNRjfDIky5yLnQIyiyE8LbUMOcQWAnszSn2XGbb9h/JLXotP703aLwLAh2eTzSFw=='
]
# rubocop:enable LineLength
