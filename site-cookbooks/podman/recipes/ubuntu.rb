return unless platform?("ubuntu")

if node["lsb"]["release"] == "22.04"
  # in another spiral of sadness, ubuntu ships with 3.4 on 22.04 and there is
  # no good ppa (unstable does not have btrfs support)
  package "old_podman_packages" do
    action :remove
    package_name %{conmon podman catatonic}
  end

  package "podman_build_deps" do
    package_name %w{
      autoconf
      automake
      btrfs-progs
      build-essential
      git
      iptables
      libassuan-dev
      libbtrfs-dev
      libc6-dev
      libcap-dev
      libdevmapper-dev
      libglib2.0-dev
      libgpg-error-dev
      libgpgme-dev
      libprotobuf-c-dev
      libprotobuf-dev
      libseccomp-dev
      libselinux1-dev
      libsystemd-dev
      libtool
      libyajl-dev
      netavark
      pkg-config
      pkgconf
      uidmap
    }
  end


  # install golang
  directory "/usr/local/go" do
    action :nothing
    recursive true
  end

  arch = case node["kernel"]["machine"]
         when "x86_64"
           "amd64"
         when "aarch64"
           "arm64"
         else
           Chef::Log.fatal("Unsupported arch #{node["kernel"]["machine"]}")
           raise
  end

  remote_file "#{Chef::Config[:file_cache_path]}/go.tar.gz" do
    source "https://go.dev/dl/go#{node["podman"]["go"]["version"]}.linux-#{arch}.tar.gz"
    notifies :run, "execute[install go]", :immediately
  end

  execute "install go" do
    action :nothing
    command "tar -C /usr/local -xzf #{Chef::Config[:file_cache_path]}/go.tar.gz"
  end


  directory "/etc/containers"

  remote_file "/etc/containers/registries.conf" do
    source "https://src.fedoraproject.org/rpms/containers-common/raw/main/f/registries.conf"
    mode "0644"
  end

  remote_file "/etc/containers/policy.json" do
    source "https://src.fedoraproject.org/rpms/containers-common/raw/main/f/default-policy.json"
    mode "0644"
  end

  include_recipe "podman::sources"

  bash "build and install podman" do
    action :nothing
    cwd "#{Chef::Config[:file_cache_path]}/podman"
    code <<~EOH
      export PATH="/usr/local/go/bin:$PATH"
      make BUILDTAGS="seccomp systemd cni"
      make install prefix=/usr
    EOH
    subscribes :run, "git[#{Chef::Config[:file_cache_path]}/podman]", :immediately
  end
else
  package "podman"
end


service "podman.socket" do
  action %i(enable start)
end
