return unless platform?("ubuntu")

if node["lsb"]["release"] == "22.04" || node["lsb"]["release"] == "24.04"
  # in another spiral of sadness, ubuntu ships with 3.4 on 22.04 and there is
  # no good ppa (unstable does not have btrfs support)
  package "old_podman_packages" do
    action :remove
    package_name %w{podman conmon crun catatonit}
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
  directory "/etc/containers/systemd"
  directory "/etc/containers/networks"

  file "/etc/containers/registries.conf" do
    mode "0644"
    content <<~EOH
      unqualified-search-registries = ["registry.fedoraproject.org", "registry.access.redhat.com", "docker.io", "quay.io"]
      short-name-mode="enforcing"
    EOH
  end

  file "/etc/containers/policy.json" do
    mode "0644"
    content <<~EOH
      {
      "default": [
          {
              "type": "insecureAcceptAnything"
          }
      ],
      "transports":
          {
              "docker-daemon":
                  {
                      "": [{"type":"insecureAcceptAnything"}]
                  }
          }
      }
    EOH
  end

  include_recipe "podman::sources"

  bash "build and install podman" do
    action :nothing
    cwd "#{Chef::Config[:file_cache_path]}/podman"
    code <<~EOH
      export PATH="/usr/local/go/bin:$PATH"
      make BUILDTAGS="seccomp systemd cni"
      PREFIX=/usr make install
    EOH
    subscribes :run, "git[#{Chef::Config[:file_cache_path]}/podman]", :immediately
  end
else
  package "podman"
end

service "podman.socket" do
  action %i(enable start)
end
