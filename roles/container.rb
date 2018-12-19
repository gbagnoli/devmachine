name 'container'
description 'common configurations for containers'

run_list [
  'recipe[datadog::dd-handler]'
]

override_attributes(
  'os-hardening' => {
    'components' => {
      'auditd' => false
    }
  }
)
