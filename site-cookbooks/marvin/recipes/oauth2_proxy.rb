include_recipe 'server::oauth2_proxy'

oauth2_proxy_site 'tigc' do
  auth_provider 'google'
  http_address '127.0.0.1:4180'
  https_address '127.0.0.1:4181'
end
