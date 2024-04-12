resource_name :podman_volume
provides :podman_volume
unified_mode true

property :name, String, name_property: true
property :volume_name, [String, NilClass], default: nil
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
default_action :create

action :create do
  config[:Volume].insert(0, "VolumeName=#{volume_name}")
  podman_systemd_unit "#{new_resource.name}.volume" do
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
  def volume_name
    if new_resource.volume_name.nil?
        new_resource.name
    else
        new_resource.volume_name
    end
  end
end
