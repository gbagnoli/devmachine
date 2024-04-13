resource_name :podman_kube
provides :podman_kube
unified_mode true

property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  podman_systemd_unit "#{new_resource.name}.kube" do
    config new_resource.config
    action :create
    triggers_reload new_resource.triggers_reload
  end
end

action :delete do
  podman_systemd_unit new_resource.name do
    action :delete
    triggers_reload new_resource.triggers_reload
  end
end

action :stop do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.kube").run_action(:stop_service)
end

action :restart do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.kube").run_action(:restart_service)
end
