# frozen_string_literal: true

resource_name :lxd_config
property :content, String, required: true
property :path, String, default: '/var/lib/lxd.yaml'

action :create do
  directory ::File.dirname(new_resource.path) do
    owner 'root'
    group 'root'
    mode '0755'
    recursive true
  end

  execute "preseed_lxd_#{new_resource.path}" do
    action :nothing
    command "lxd init --preseed < #{new_resource.path}"
  end

  file new_resource.path do
    content new_resource.content
    notifies :run, "execute[preseed_lxd_#{new_resource.path}]", :immediately
  end
end
