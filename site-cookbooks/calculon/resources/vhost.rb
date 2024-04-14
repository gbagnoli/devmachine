resource_name :calculon_vhost
provides :calculon_vhost
unified_mode true

property :server_name, [Array, String]
property :upstream_url, [String]
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
property :disable_default_location, [true, false], default: false
property :cloudflare, [true, false], default: true
property :extra_config, [String, NilClass]
property :maps, [Array, NilClass]
property :proxy_caches, [Hash, NilClass]

action :create do
  server_name = Array(new_resource.server_name).map(&:to_s)
  server_name.each do |name|
    calculon_acme_certificate name
  end

  directory www_directory do
    mode "0755"
  end

  template vhost_conf_file do
    source "vhost.erb"
    variables(
      vhost: new_resource.name,
      server_name: server_name,
      upstream_url: upstream_url,
      certificate_key: certificate_key,
      certificate_file: certificate_file,
      disable_default_location: new_resource.disable_default_location,
      cloudflare: new_resource.cloudflare,
      www_directory: www_directory,
      access_log: access_log,
      error_log: error_log,
      extra_config: new_resource.extra_config,
      maps: new_resource.maps,
      proxy_caches: new_resource.proxy_caches,
    )
    notifies :reload, "service[nginx]", :immediately
  end
end

action :delete do
  file certificate_file do
    action :delete
  end

  file certificate_key do
    action :delete
  end

  file vhost_conf_file do
    action :delete
    notifies :reload, "service[nginx]", :immediately
  end
end

action_class do
  def cert_root
    "#{node["calculon"]["acme"]["certs_dir"]}/certificates/#{new_resource.name}"
  end

  def certificate_file
    "#{cert_root}.crt"
  end

  def certificate_key
    "#{cert_root}.key"
  end

  def vhost_conf_file
    "/etc/nginx/conf.d/#{new_resource.name}.conf"
  end

  def www_directory
    "/var/www/#{new_resource.name}"
  end

  def upstream_url
    "#{new_resource.upstream_protocol}://#{new_resource.upstream_url}"
  end

  def access_log
    "/var/log/nginx/vhosts/#{new_resource.name}.access.log"
  end

  def error_log
    "/var/log/nginx/vhosts/#{new_resource.name}.error.log"
  end
end
