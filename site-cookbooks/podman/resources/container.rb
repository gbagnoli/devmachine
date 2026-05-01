resource_name :podman_container
provides :podman_container
unified_mode true

property :container_name, [String, NilClass]
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
property :auto_update, [true, false], default: true
property :pull, String, default: "never"
default_action :create

action :create do
  new_resource.config[:Container].insert(0, "ContainerName=#{container_name}")
  if new_resource.auto_update
    new_resource.config[:Container].insert(1, "AutoUpdate=registry")
  end

  unless new_resource.pull.empty?
    new_resource.config[:Container].insert(1, "Pull=#{new_resource.pull}")
  end

  unless image_name.end_with?(".image")
    execute "pull_podman_image_for_container_#{container_name}" do
      not_if "/usr/bin/podman image exists #{image_name}"
      command "/usr/bin/podman pull #{image_name}"
    end
  end

  podman_systemd_unit "#{new_resource.name}.container" do
    config new_resource.config
    action :create
    triggers_reload new_resource.triggers_reload
  end
end

action :delete do
  podman_systemd_unit "#{new_resource.name}.container" do
    action :delete
    config new_resource.config
    triggers_reload new_resource.triggers_reload
  end

  unless image_name.end_with?(".image")
    execute "rm_podman_image_for_container_#{container_name}" do
      only_if `/usr/bin/podman image exists #{image_name}`
      not_if `/usr/bin/podman ps -a --external --filter image=#{image_name} --format "{{.ID}}" | grep -q .`
      command `/usr/bin/podman image remove #{image_name}`
    end
  end
end

action :stop do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.container").run_action(:stop_service)
end

action :restart do
  find_resource(:podman_systemd_unit, "#{new_resource.name}.container").run_action(:restart_service)
end

action_class do
  def container_name
    if new_resource.container_name.nil?
        new_resource.name
    else
        new_resource.container_name
    end
  end

  def image_name
    new_resource.config[:Container].find { |s| s.start_with?("Image=") }&.delete_prefix("Image=")
  end
end
