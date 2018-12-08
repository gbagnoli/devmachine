default['marvin']['thelounge']['userid'] = '2000'
default['marvin']['thelounge']['groupid'] = '2000'
default['marvin']['thelounge']['home'] = '/var/lib/thelounge'
default['marvin']['thelounge']['port'] = 6677

default['marvin']['oauth2_proxy']['http_port'] = '4180'
default['marvin']['oauth2_proxy']['upstream_port'] = 4189
default['marvin']['oauth2_proxy']['auth_provider'] = 'google'
default['marvin']['oauth2_proxy']['redirect-url'] = 'https://www.tigc.eu/oauth2/callback'
# need to set this in the secrets file
default['marvin']['oauth2_proxy']['authenticated_emails'] = nil
default['marvin']['oauth2_proxy']['client-secret'] = nil
default['marvin']['oauth2_proxy']['client-id'] = nil
default['marvin']['oauth2_proxy']['cookie-secret'] = nil
