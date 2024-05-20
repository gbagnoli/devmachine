resource_name :podman_image
provides :podman_image
unified_mode true

property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create


action :create do
  new_resource.config[:Unit] ||= [
    "Description=#{new_resource.name} image",
    "After=network-online.target",
  ]
  new_resource.config[:Install] ||= [
    "WantedBy=multi-user.target default.target"
  ]
  podman_systemd_unit "#{new_resource.name}.image" do
    config new_resource.config
    action :create
    restart_service false
    triggers_reload new_resource.triggers_reload
  end
end

action :delete do
  podman_systemd_unit "#{new_resource.name}.image" do
    config new_resource.config
    action :delete
    triggers_reload new_resource.triggers_reload
  end
end
