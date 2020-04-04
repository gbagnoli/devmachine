node.override["server"]["chef"]["cron"]["minute"] = "10"
node.override["dnscrypt_proxy"]["listen_port"] = "54"

include_recipe "pentolino::openvpn"
include_recipe "nginx"
