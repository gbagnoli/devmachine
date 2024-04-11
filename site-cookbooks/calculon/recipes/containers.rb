if platform?("rocky")
  # in a spiral of sadness, rocky's podman
  # doesn't have btrfs enabled
  # so install the version in the repo, lock it down
  # then compile podman (!!!) and substitute the files
  package "podman" do
    package_name %w{podman crun python3-dnf-plugin-versionlock.noarch}
  end

  podman = node["calculon"]["rocky"]["podman"]["git"]
  crun = node["calculon"]["rocky"]["crun"]["git"]

  package "podman deps" do
    package_name podman[:deps]
  end

  package "crun deps" do
    package_name crun[:deps]
  end

  execute "lock crun" do
    command "dnf versionlock crun"
    not_if "dnf versionlock crun | grep -q crun"
  end

  podman[:download].each do |spec|
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

  git "#{Chef::Config[:file_cache_path]}/podman" do
    repository podman[:url]
    action :sync
    user "root"
    revision podman[:tag]
    notifies :run, "bash[build and install podman]", :immediately
  end

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
  end

  git "#{Chef::Config[:file_cache_path]}/crun" do
    repository crun[:url]
    action :sync
    user "root"
    revision crun[:tag]
    notifies :run, "bash[build and install crun]", :immediately
  end

  bash "build and install crun" do
    action :nothing
    cwd "#{Chef::Config[:file_cache_path]}/crun"
    code <<-EOH
      ./autogen.sh
      ./configure --prefix=/usr
      make -j8
      make install
    EOH
  end
else
  package "virt" do
    package_name %w(crun podman)
  end
end

execute "reset podman" do
  command "podman system reset -f"
  action :nothing
end

path = node["calculon"]["containers"]["storage"]["volume"]

execute "create subvolume at #{path}" do
  command "btrfs subvolume create #{path}"
  not_if "btrfs subvolume show #{path} &>/dev/null"
end

template "/etc/containers/storage.conf" do
  variables node["calculon"]["containers"]["storage"]
  source "podman_storage.conf.erb"
  owner "root"
  group "root"
  mode "0755"
  notifies :run, "execute[reset podman]", :before
end
