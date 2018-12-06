include_recipe 'bender::base'
include_recipe 'bender::users'
include_recipe 'bender::containers'
include_recipe 'bender::firewall'
include_recipe 'bender::nginx'

include_recipe 'datadog::dd-agent'
include_recipe 'datadog::dd-handler'
