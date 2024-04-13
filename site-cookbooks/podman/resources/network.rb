resource_name :podman_network
provides :podman_network
unified_mode true

property :network_name, [String, NilClass]
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  new_resource.config[:Network].insert(0, "NetworkName=#{network_name}")
  podman_systemd_unit "#{new_resource.name}.network" do
    config new_resource.config
    action :create
    triggers_reload new_resource.triggers_reload
    # updates on network files are not even supported anyway
    restart_service false
  end
end

action :stop do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.network").run_action(:stop_service)
end

action :restart do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.network").run_action(:restart_service)
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
