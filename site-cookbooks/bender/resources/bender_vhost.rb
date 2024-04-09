resource_name :bender_vhost
provides :bender_vhost
unified_mode true

file_check = {
  "file should exists" => lambda { |path|
    ::File.exist?(path)
  },
}

container_check = {
  "container should exists" => lambda { |name|
    name.nil? || node["bender"]["containers"].key?(name)
  },
}

property :vhost_name, String, name_property: true
property :server_name, [Array, String], required: true
property :port, [Integer, NilClass], default: nil
property :disable_default_location, [true, false], default: false
property :upstream_url, [String, NilClass]
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
property :container, [String, NilClass], callbacks: container_check
property :ssl, [true, false], default: false
property :letsencrypt, [true, false], default: false
property :letsencrypt_common_name, [String, NilClass]
property :letsencrypt_contact, [String, NilClass]
property :letsencrypt_alt_names, [Array, NilClass], default: nil
property :ssl_cert_path, [String, NilClass], callbacks: file_check
property :ssl_key_path, [String, NilClass], callbacks: file_check
property :cloudflare, [true, false], default: false
property :extra_config, [String, NilClass]
property :maps, [Array, NilClass]
property :proxy_caches, [Hash, NilClass]

action :create do
  port = new_resource.ssl ? "443" : new_resource.port || "80"

  template "/etc/nginx/sites-enabled/#{new_resource.vhost_name}" do
    source "nginx/vhost.erb"
    variables(
      vhost: new_resource.vhost_name,
      port: port,
      server_name: server_names,
      disable_default_location: new_resource.disable_default_location,
      upstream_url: upstream_url,
      certificate_key: certificate_key,
      certificate_file: certificate_file,
      ssl: new_resource.ssl,
      letsencrypt: new_resource.letsencrypt,
      cloudflare: new_resource.cloudflare,
      www_directory: www_directory,
      extra_config: new_resource.extra_config,
      maps: new_resource.maps,
      proxy_caches: new_resource.proxy_caches,
    )
    notifies :reload, "service[nginx]", :immediately
  end

  if new_resource.letsencrypt
    include_recipe "acme"
    directory www_directory
    directory certificate_directory do
      mode "0700"
    end

    acme_certificate letsencrypt_common_name do
      crt certificate_file
      key certificate_key
      wwwroot www_directory
      contact new_resource.letsencrypt_contact if new_resource.letsencrypt_contact
      alt_names new_resource.letsencrypt_alt_names if new_resource.letsencrypt_alt_names
      notifies :create, "template[/etc/nginx/sites-enabled/#{new_resource.vhost_name}]"
    end
  end
end

action_class do
  def www_directory
    "/var/www/#{new_resource.vhost_name}"
  end

  def server_name
    new_resource.server_name.is_a?(Array) ? new_resource.server_name : [new_resource.server_name]
  end

  def letsencrypt_common_name
    new_resource.letsencrypt_common_name || server_name.first
  end

  def certificate_directory
    dir = node["bender"]["certificates"]["directory"]
    dir = "#{dir}/#{new_resource.container}" unless new_resource.container.nil?
    dir
  end

  def certificate_file
    new_resource.ssl_cert_path || "#{certificate_directory}/#{new_resource.vhost_name}.crt"
  end

  def certificate_key
    new_resource.ssl_key_path || "#{certificate_directory}/#{new_resource.vhost_name}.key"
  end

  def upstream_url
    if !new_resource.disable_default_location && new_resource.upstream_url.nil? && new_resource.container.nil?
      raise ArgumentError, "either upstream_url or container parameters required"
    end

    up = new_resource.upstream_url || "#{new_resource.container}.lxd"
    "#{new_resource.upstream_protocol}://#{up}"
  end

  def server_names
    (server_name + (new_resource.letsencrypt_alt_names || [])).join(" ")
  end
end
