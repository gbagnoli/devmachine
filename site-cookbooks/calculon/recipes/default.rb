include_recipe "calculon::repos"
include_recipe "calculon::monitoring"
include_recipe "calculon::base"
include_recipe "calculon::nginx"
include_recipe "calculon::lego"
include_recipe "calculon::monitoring"

include_recipe "calculon::tailscale"
include_recipe "calculon::containers"
