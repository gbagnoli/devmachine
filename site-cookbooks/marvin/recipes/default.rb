node.override["server"]["chef"]["cron"]["minute"] = "10"
node.override["dnscrypt_proxy"]["listen_port"] = "54"

include_recipe "marvin::openvpn"
include_recipe "dnscrypt_proxy"

nginx_install 'nginx' do
  source 'repo'
end

nginx_service 'nginx' do
  action :enable
  delayed_action :start
end

include_recipe "marvin::oauth2_proxy"
include_recipe "marvin::thelounge"
include_recipe "marvin::syncthing"
