resource_name :gnome_extension
provides :gnome_extension
unified_mode true

property :repository, String, required: true
property :install_script, String, required: true
property :deps, [Array, NilClass], default: []
property :revision, String, default: "master"
default_action %I(install enable)

action :install do
  # specify all paths so we can set permissions
  [install_base_dir, src_base_dir].each do |dir|
    directory dir do
      mode "0755"
      recursive true
    end
  end

  if new_resource.deps && !new_resource.deps.empty?
    package "install_gnome_extension_#{new_resource.name}_deps" do
      package_name new_resource.deps
      action :install
    end
  end

  git src_dir do
    repository new_resource.repository
    revision new_resource.revision
    action :sync
    notifies :run, "bash[install_gnome_extension_#{new_resource.name}]", :immediately
  end

  bash "install_gnome_extension_#{new_resource.name}" do
    action :nothing
    cwd src_dir
    code formatted_install_script
  end
end

action :enable do
  file "/etc/dconf/profile/user" do
    mode "0755"
    content <<-EOH
    user-db:user
    system-db:local
    EOH
  end

  directory "/etc/dconf/db/local.d" do
    mode "0755"
    recursive true
  end

  file dconf_extension_path do
    mode "0644"
    content <<-EOH
    [org/gnome/shell]
    enabled-extensions=['#{new_resource.name}']
    EOH
    notifies :run, "execute[dconf_update]", :delayed
  end

  execute "dconf_update" do
    action :nothing
    command "dconf update"
  end
end

action :disable do
  file dconf_extension_path do
    action :delete
    notifies :run, "execute[dconf_update]", :delayed
  end

  execute "dconf_update" do
    action :nothing
    command "dconf update"
  end
end

action :uninstall do
  directory install_dir do
    action :delete
    recursive true
  end
end


action_class do
  def install_base_dir
    "/usr/share/gnome-shell/extensions"
  end

  def src_base_dir
    "/usr/src/gnome-shell/extensions"
  end

  def install_dir
    "#{install_base_dir}/#{new_resource.name}"
  end

  def src_dir
    "#{src_base_dir}/#{new_resource.name}"
  end

  def formatted_install_script
    format(
      new_resource.install_script,
      install_dir: install_dir,
      src_dir: src_dir,
      name: new_resource.name
    )
  end

  def dconf_extension_path
    "/etc/dconf/db/local.d/00-extensions-#{new_resource.name}"
  end
end
