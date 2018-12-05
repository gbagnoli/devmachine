default['bender']['certificates']['directory'] = '/etc/ssl/containers'

default['bender']['vhosts']['bender.tigc.eu'] = {
  # for nginx. either a string, or an array of strings
  server_name: 'bender.tigc.eu',
  # either you set the container, or set upstream_url
  container: 'marvin',
  # for letsencrypt you also need ssl: true
  ssl: true,
  letsencrypt: true,
  # if empty it will use the first server_name
  #  letsencrypt_common_name: 'bender.tigc.eu',
  letsencrypt_alt_names: ['bender.test.tigc.eu'],
  # restricts real-ip headers from cloudflare ips
  cloudflare: true

  # other options:
  # port (best left alone)
  # letsencrypt_contact (The contact to use for the certificate)
}
