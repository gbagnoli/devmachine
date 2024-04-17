resource_name :calculon_acme_certificate
provides :calculon_acme_certificate
unified_mode true

default_action :request

action :request do
  include_recipe "calculon::nginx"

  execute "request_certificate_#{new_resource.name}" do
    command "/usr/local/bin/lego_request #{new_resource.name}"
    not_if { ::File.exist?("#{node["calculon"]["acme"]["certs_dir"]}/certificates/#{new_resource.name}.crt") }
  end
end
