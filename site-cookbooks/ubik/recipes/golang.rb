remote_file "#{Chef::Config[:file_cache_path]}/godeb-amd64.tar.gz" do
  source "https://godeb.s3.amazonaws.com/godeb-amd64.tar.gz"
  notifies :run, "execute[unpack godeb]", :immediately
end

execute "unpack godeb" do
  command "tar xvf #{Chef::Config[:file_cache_path]}/godeb-amd64.tar.gz -C /usr/local/bin"
  action :nothing
end

version = node["ubik"]["golang"]["version"]
execute "install golang" do
  command "/usr/local/bin/godeb install #{version}"
  cwd Chef::Config[:file_cache_path]
  not_if "go version 2>/dev/null | grep -q go#{version}"
end
