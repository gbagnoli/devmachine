# frozen_string_literal: true

resource_name :lxd_config
property :hostname, String, name_property: true
property :content, String, required: true
default_action :create

action :create do
  include_recipe "lxd::default"
  directory ::File.dirname(config_path) do
    owner "root"
    group "root"
    mode "0755"
    recursive true
  end

  execute "preseed_lxd_#{config_path}" do
    action :nothing
    command "lxd init --preseed < #{config_path}"
  end

  file config_path do
    content new_resource.content
    notifies :run, "execute[preseed_lxd_#{config_path}]", :immediately
  end
end

action_class do
  def config_path
    "#{node["lxd"]["config_dir"]}/#{new_resource.hostname}.yaml"
  end
end
