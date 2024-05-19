resource_name :podman_nginx_acme_certificate
provides :podman_nginx_acme_certificate
unified_mode true

default_action :request

action :request do
  include_recipe "podman_nginx::acme"
  certs_dir = node["podman"]["nginx"]["acme"]["certs_dir"]

  execute "request_certificate_#{new_resource.name}" do
    command "/usr/local/bin/lego_request #{new_resource.name}"
    not_if { ::File.exist?("#{certs_dir}/certificates/#{new_resource.name}.crt") }
  end
end
