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

  directory local_vhost_root do
    mode "0755"
  end

  directory local_cache do
    mode "0755"
    user node["calculon"]["nginx"]["user"]
    group node["calculon"]["nginx"]["group"]
  end

  template local_vhost_conf_file do
    source "vhost.erb"
    variables(
      paths: node["calculon"]["nginx"]["container"],
      vhost: new_resource.name,
      server_name: server_name,
      upstream_url: upstream_url,
      certificate_key: container_certificate_key,
      certificate_file: container_certificate_file,
      disable_default_location: new_resource.disable_default_location,
      cloudflare: new_resource.cloudflare,
      www_directory: container_www,
      cache_directory: container_cache,
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

  file local_vhost_conf_file do
    action :delete
    notifies :reload, "service[nginx]", :immediately
  end

  directory local_cache do
    action :remove
    recursive true
  end

  directory local_vhost_root do
    action :remove
    recursive true
  end
end

action_class do

  def local_paths
    node["calculon"]["storage"]["paths"]
  end

  def container_paths
    node["calculon"]["nginx"]["container"]
  end

  def cert_root
    "#{node["calculon"]["acme"]["certs_dir"]}/certificates/#{new_resource.name}"
  end

  def container_cert_root
    "#{container_paths["ssl"]}/#{new_resource.name}"
  end

  def container_www
    "#{container_paths["www"]}/#{new_resource.name}"
  end

  def container_cache
    "#{container_paths["cache"]}/#{new_resource.name}"
  end

  def local_vhost_root
    "#{local_paths["www"]}/vhosts/#{new_resource.name}"
  end

  def local_cache
    "#{local_paths["www"]}/cache/#{new_resource.name}"
  end

  def container_certificate_file
    "#{container_cert_root}.crt"
  end

  def container_certificate_key
    "#{container_cert_root}.key"
  end

  def certificate_file
    "#{cert_root}.crt"
  end

  def certificate_key
    "#{cert_root}.key"
  end

  def local_vhost_conf_file
    "#{local_paths["www"]}/etc/conf.d/#{new_resource.name}.conf"
  end

  def upstream_url
    "#{new_resource.upstream_protocol}://#{new_resource.upstream_url}"
  end

  def access_log
    "#{container_paths["logs"]}/#{new_resource.name}.access.log"
  end

  def error_log
    "#{container_paths["logs"]}/#{new_resource.name}.error.log"
  end
end
