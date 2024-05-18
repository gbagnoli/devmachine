podman = node["podman"]["rocky"]["podman"]["git"]
crun = node["podman"]["rocky"]["crun"]["git"]

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

git "#{Chef::Config[:file_cache_path]}/catatonic" do
  repository node["catatonic"]["git"]
  action :sync
  user "root"
  notifies :run, "bash[build and install catatonic]", :immediately
end

bash "build and install catatonic" do
  action :nothing
  cwd "#{Chef::Config[:file_cache_path]}/catatonic"
  code <<~EOH
    ./autogen.sh
    ./configure --prefix=/usr
    make -j8
    make install
  EOH
end

git "#{Chef::Config[:file_cache_path]}/podman" do
  repository podman[:url]
  action :sync
  user "root"
  revision podman[:tag]
end
