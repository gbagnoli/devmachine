resource_name :calculon_oauth2_proxy
provides :calculon_oauth2_proxy
unified_mode true

property :emails, Array, required: true
property :redirect_url, String, required: true
property :port, [Integer, String], required: true
property :upstream_port, [Integer, String], required: true
property :upstream_address, String, default: "[::1]"
property :upstream_protocol, String, default: "http", equal_to: %w(http https)
property :address, String, default: "[::]"
property :auth_provider, String, default: "google"

default_action :create

action :create do
  include_recipe "calculon::oauth2_proxy"
  require 'toml'

  file emails_file do
    content new_resource.emails.sort.join("\n")
    mode '400'
    owner "oauth2proxy"
    notifies :restart, "service[oauth2-proxy-#{new_resource.name}]"
  end

  file config_file do
    content toml_config
    mode '400'
    owner "oauth2proxy"
    notifies :restart, "service[oauth2-proxy-#{new_resource.name}]"
  end

  podman_container "oauth2-proxy-#{new_resource.name}" do
    config(
      Container: %W{
        Pod=web.pod
        Image=oauth2_proxy.image
        Volume=#{emails_file}:#{emails_file}:ro
        Volume=#{config_file}:#{config_file}:ro
        Exec=--config=#{config_file}
      },
      Unit: [
        "Description=Start oauth2 proxy for #{new_resource.name}",
        "After=network-online.target",
        "Wants=network-online.target",
      ],
      Install: %w{
        WantedBy=multi-user.target
      }
    )
  end

  service "oauth2-proxy-#{new_resource.name}" do
    action :nothing
  end
end

action :remove do
  podman_container "oauth2-proxy-#{new_resource.name}" do
    action :delete
    config nil.to_h
  end
  [emails_file, config_file].each do |f|
    file f do
      action :delete
    end
  end
end

action_class do
  def emails_file
    "/etc/oauth2_proxy/#{new_resource.name}.emails.txt"
  end

  def config_file
    "/etc/oauth2_proxy/#{new_resource.name}.config.cfg"
  end

  def toml_config
    TOML::Generator.new(config_hash).body
  end

  def config_hash
    {
      provider: new_resource.auth_provider,
      http_address: http_address ,
      upstreams: upstreams,
      redirect_url: new_resource.redirect_url,
      authenticated_emails_file: emails_file,
      cookie_secret: secrets["cookie-secret"],
      client_id: secrets["client-id"],
      client_secret: secrets["client-secret"],
    }
  end

  def http_address
    "#{new_resource.address}:#{new_resource.port}"
  end

  def upstreams
    ["#{new_resource.upstream_protocol}://#{new_resource.upstream_address}:#{new_resource.upstream_port}"]
  end

  def secrets
    node["calculon"]["oauth2_proxy"]["secrets"]
  end
end
