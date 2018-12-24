name 'container'
description 'common configurations for containers'

run_list [
  'server::container'
]

override_attributes(
  'os-hardening' => {
    'components' => {
      'auditd' => false
    }
  }
)
