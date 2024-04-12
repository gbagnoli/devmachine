resource_name :podman_network
provides :podman_network
unified_mode true

property :name, String, name_property: true
property :network_name, [String, NilClass], default: nil
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  new_resource.config[:Network].insert(0, "NetworkName=#{network_name}")
  podman_systemd_unit "#{new_resource.name}.network" do
    config new_resource.config
    action :create
    triggers_reload new_resource.triggers_reload
  end
end

action :start do
  systemd_unit "#{new_resource.name}-network.service" do
    action :start
  end
end

action :delete do
  podman_systemd_unit new_resource.name do
    type :network
    action :delete
    triggers_reload new_resource.triggers_reload
  end

  execute "podman_delete_network_#{new_resource.name}" do
    command "podman network rm #{network_name}"
    action :run
  end
end

action_class do
  def network_name
    if new_resource.network_name.nil?
        new_resource.name
    else
        new_resource.network_name
    end
  end
end
