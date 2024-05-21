resource_name :podman_nginx_vhost
provides :podman_nginx_vhost
unified_mode true

property :server_name, [Array, String]
property :upstream_address, String, default: "[::1]"
property :upstream_port, [String, Integer], default: 8080
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
property :disable_default_location, [true, false], default: false
property :cloudflare, [true, false], default: true
property :oauth2_proxy, [Hash, NilClass], default: nil
property :extra_config, [String, NilClass]
property :maps, [Array, NilClass]
property :proxy_caches, [Hash, NilClass]
property :upstream_paths, Hash, default: {}
property :act_as_upstream, [String, Integer, NilClass]
property :extra_config_as_upstream, [String, NilClass]
property :upgrade, [true, false], default: true


action :create do
  server_name = Array(new_resource.server_name).map(&:to_s)
  server_name.each do |name|
    podman_nginx_acme_certificate name
  end

  # handle the default oauth2 proxy if any
  unless new_resource.oauth2_proxy.nil?
    conf = new_resource.oauth2_proxy
     podman_nginx_oauth2_proxy new_resource.name do
       emails conf[:emails]
       port conf[:port]
       redirect_url conf[:redirect_url] || "https://#{server_name.first}/oauth2/callback"

       # these are optional
       address "[::1]"
       auth_provider conf[:auth_provider]

       upstream_port oauth2_proxy_upstream_port
       upstream_address oauth2_proxy_upstream_address
       upstream_protocol new_resource.upstream_protocol
     end
  end

  directory local_vhost_root do
    mode "0755"
  end

  directory local_cache do
    mode "0755"
    user node["podman"]["nginx"]["user"]
    group node["podman"]["nginx"]["group"]
  end

  template local_vhost_conf_file do
    cookbook "podman_nginx"
    source "vhost.erb"
    variables(
      paths: container_paths,
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
      upstream_paths: new_resource.upstream_paths,
      act_as_upstream: new_resource.act_as_upstream,
      extra_config_as_upstream: new_resource.extra_config_as_upstream,
      upgrade: new_resource.upgrade,
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
    action :delete
    recursive true
  end

  directory local_vhost_root do
    action :delete
    recursive true
  end
  unless new_resource.oauth2_proxy.nil?
    podman_nginx_oauth2_proxy new_resource.name do
      emails ["notimportant"]
      port 1111
      redirect_url "notimportant"
      action :remove
      upstream_port 1112
    end
  end
end

action_class do

  def local_path
    node["podman"]["nginx"]["path"]
  end

  def container_paths
    node["podman"]["nginx"]["container"]
  end

  def cert_root
    "#{node["podman"]["nginx"]["acme"]["certs_dir"]}/certificates/#{new_resource.name}"
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
    "#{local_path}/vhosts/#{new_resource.name}"
  end

  def local_cache
    "#{local_path}/cache/#{new_resource.name}"
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
    "#{local_path}/etc/conf.d/#{new_resource.name}.conf"
  end

  def upstream_url
    if new_resource.oauth2_proxy.nil?
      "#{new_resource.upstream_protocol}://#{new_resource.upstream_address}:#{new_resource.upstream_port}"
    else
      port = new_resource.oauth2_proxy[:port]
      "http://[::1]:#{port}"
    end
  end

  def oauth2_proxy_upstream_port
    if new_resource.act_as_upstream.nil?
      new_resource.upstream_port
    else
      new_resource.act_as_upstream
    end
  end

  def oauth2_proxy_upstream_address
    if new_resource.act_as_upstream.nil?
      new_resource.upstream_address
    else
     "[::1]"
    end
  end

  def access_log
    "#{container_paths["logs"]}/#{new_resource.name}.access.log"
  end

  def error_log
    "#{container_paths["logs"]}/#{new_resource.name}.error.log"
  end
end
