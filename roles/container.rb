name 'container'
description 'common configurations for containers'

override_attributes(
  'os-hardening' => {
    'components' => {
      'auditd' => false
    }
  }
)
