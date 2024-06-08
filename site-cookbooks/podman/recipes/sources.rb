package "podman_deps" do
  package_name node["podman"]["sources"].flat_map {|_, conf| conf[:deps]}.uniq
end

node["podman"]["sources"].each do |app, conf|
  git "#{Chef::Config[:file_cache_path]}/#{app}" do
    repository conf[:url]
    action :sync
    user "root"
    revision conf[:tag]
  end

  next if app == "podman"

end

bash "build and install conmon" do
  action :nothing
  cwd "#{Chef::Config[:file_cache_path]}/conmon"
  code <<-EOH
  make
  PREFIX=/usr make install
  PREFIX=/usr make podman
  PREFIX=/usr make crio
  EOH
  subscribes :run, "git[#{Chef::Config[:file_cache_path]}/conmon]", :immediately
end

%w{crun catatonit}.each do |app|
  bash "build and install #{app}" do
    action :nothing
    cwd "#{Chef::Config[:file_cache_path]}/#{app}"
    code <<-EOH
    ./autogen.sh
    ./configure --prefix=/usr
    make
    PREFIX=/usr make install
    EOH
    subscribes :run, "git[#{Chef::Config[:file_cache_path]}/#{app}]", :immediately
  end
end
