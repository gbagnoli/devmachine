return unless platform?("rocky")

# in a spiral of sadness, rocky's podman
# doesn't have btrfs enabled
# so install the version in the repo, lock it down
# then compile podman (!!!) and substitute the files
package "podman" do
  package_name %w{podman crun python3-dnf-plugin-versionlock.noarch}
end

execute "lock crun" do
  command "dnf versionlock crun"
  not_if "dnf versionlock crun | grep -q crun"
end

node["podman"]["sources"]["podman"]["rpms"].each do |spec|
  spec[:rpms].each do |rpm|
    local = "#{Chef::Config[:file_cache_path]}/#{rpm}"
    remote = "#{spec[:url]}/#{rpm}"
    remote_file local do
      source remote
      notifies :run, "execute[install_#{rpm}]", :immediately
    end

    execute "install_#{rpm}" do
      action :nothing
      command "dnf install --assumeyes #{local}"
    end
  end
end

include_recipe "podman::sources"

bash "build and install podman" do
  action :nothing
  cwd "#{Chef::Config[:file_cache_path]}/podman"
  code <<-EOH
    make
    make rpm
    dnf install --assumeyes rpm/RPMS/*/*.rpm
    git checkout rpm/podman.spec
    git rm -r rpm/BUILD rpm/podman-.*.tar.gz
  EOH
  subscribes :run, "git[#{Chef::Config[:file_cache_path]}/podman]", :immediately
end
