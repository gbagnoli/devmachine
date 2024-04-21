resource_name :podman_systemd_unit
provides :podman_systemd_unit
unified_mode true

property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
property :restart_service, [true, false], default: true
property :user, [String, NilClass]
default_action :create

action :create do
  user = user_info
  template unit_path do
    source "podman_systemd_unit.erb"
    cookbook "podman"
    owner user.uid
    group user.uid
    variables(
      config: new_resource.config
    )
    notifies :run, reload_resource, :immediately if new_resource.triggers_reload
    notifies :restart, service_unit if service? && new_resource.restart_service
  end

  service_unit.run_action(:start) if service?
end

action :start_service do
  service_unit.run_action(:start) if service?
end

action :stop_service do
  service_unit.run_action(:stop) if service?
end

action :restart_service do
  service_unit.run_action(:restart) if service?
end

action :delete do
  file unit_path do
    action :delete
    notifies :run, reload_resource, :immediately if new_resource.triggers_reload
    notities :stop, service_unit, :before if service?(new_resource)
  end
end

action_class do
  def unit_path
    "#{configuration_dir}/#{new_resource.name}"
  end

  def configuration_dir_base
    "/etc/containers/systemd"
  end

  def configuration_dir_user
    udir = "#{configuration_dir_base}/users"
    directory udir
    udir
  end

  def user_info
    username = if new_resource.user.nil?
                  "root"
               else
                 new_resource.user
               end
    ::Etc.getpwnam(username)
  end

  def configuration_dir
    if new_resource.user.nil?
      configuration_dir_base
    else
      info = user_info
      dir = "#{configuration_dir_user}/#{info.uid}"
      directory dir do
        owner info.gid
        group info.gid
      end
    end
  end

  def service_unit
    create_service_unit unless service_unit_exists?
    service_unit_resource
  end

  def service?
    _, type = extract_service_name_and_type
    return false if %w(volume image).include? type

    true
  end

  def reload_resource
    find_resource!(:execute, "podman_reload_systemd")
  rescue Chef::Exceptions::ResourceNotFound
    declare_resource(:execute, "podman_reload_systemd") do
      command "systemctl daemon-reload"
      action :nothing
    end
  end

  def create_service_unit
    with_run_context(:root) do
      declare_resource(:systemd_unit, service_unit_name)
    end
  end

  def service_unit_resource
    find_resource(:systemd_unit, service_unit_name)
  end

  def service_unit_exists?
    !service_unit_resource.nil?
  rescue Chef::Exceptions::ResourceNotFound
    false
  end

  def service_unit_name
    name, type = extract_service_name_and_type
    return name if type == "container"

    "#{name}-#{type}.service"
  end

  def extract_service_name_and_type
    name, _, type = new_resource.name.rpartition(".")
    unless %w(container image kube network pod volume).include? type
      raise "Invalid unit type #{type} in #{new_resource.name}"
    end

    [name, type]
  end
end
