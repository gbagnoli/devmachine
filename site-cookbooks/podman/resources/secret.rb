resource_name :podman_secret
provides :podman_secret
unified_mode true

property :value, [String, Integer], required: true
default_action :create

action :create do
  execute "podman_secret_create_#{new_resource.name}" do
    command "podman secret create #{new_resource.name} - <<< $SECRET"
    environment({"SECRET" => new_resource.value})
    not_if secret_exists?
  end
end

action :delete do
  execute "podman_secret_delete_#{new_resource.name}" do
    command "podman secret rm #{new_resource.name}"
    only_if secret_exists?
    sensitive true
  end
end

action_class do
  def secret_exists?
    "podman secret ls -f Name=#{new_resource.name} --format '{{.Name}}' | grep -q #{new_resource.name}"
  end
end
