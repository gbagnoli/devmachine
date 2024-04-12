resource_name :podman_systemd_unit
provides :podman_systemd_unit
unified_mode true

property :name, String, name_property: true
property :config, Hash, required: true
property :triggers_reload, [true, false], default: true
property :user, [String, NilClass], default: nil
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
  end

  maybe_reload_systemd
end

action :delete do
  file unit_path do
    action :delete
  end

  maybe_reload_systemd
end

action_class do
  def maybe_reload_systemd
    return unless new_resource.triggers_reload

    execute "reload_systemd_podman_#{new_resource.name}" do
      command 'systemctl daemon-reload'
    end
  end

  def type
    t = new_resouce.name.split(".")[1]
    unless %w(container image kube network pod volume).include? t
      raise "Invalid unit type #{t} in #{new_resource.name}"
    end

    t
  end

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
end
