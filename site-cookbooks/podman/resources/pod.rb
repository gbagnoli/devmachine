resource_name :podman_pod
provides :podman_pod
unified_mode true

property :name, String, name_property: true
property :pod_name, [String, NilClass], default: nil
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  config[:Pod].insert(0, "PodName=#{pod_name}")
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

action_class do
  def pod_name
    if new_resource.pod_name.nil?
        new_resource.name
    else
        new_resource.pod_name
    end
  end
end
