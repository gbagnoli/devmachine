resource_name :podman_pod
provides :podman_pod
unified_mode true

property :pod_name, [String, NilClass]
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  new_resource.config[:Pod].insert(0, "PodName=#{pod_name}")
  new_resource.config[:Unit] ||= [
    "Description=#{pod_name} pod",
    "After=network-online.target",
  ]
  new_resource.config[:Install] ||= [
    "WantedBy=multi-user.target default.target"
  ]
  podman_systemd_unit "#{new_resource.name}.pod" do
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
  find_resource(:podman_systemd_unit, "#{new_resource.name}.pod").run_action(:stop_service)
end

action :restart do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.pod").run_action(:restart_service)
end

action_class do
  def pod_name
    if new_resource.pod_name.nil?
        new_resource.name
    else
        new_resource.pod_name
    end
  end
end
