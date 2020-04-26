# frozen_string_literal: true

directory "/var/cache/chef" do
  recursive true
end

remote_file "/var/cache/chef/godeb-amd64.tar.gz" do
  source "https://godeb.s3.amazonaws.com/godeb-amd64.tar.gz"
  checksum "f2b3445dda98b31a32381036bf01c7e82df1a5a151e7e838ca1f0d1fb8e80952"
  notifies :run, "execute[unpack godeb]", :immediately
end

execute "unpack godeb" do
  command "tar xvf /var/cache/chef/godeb-amd64.tar.gz -C /usr/local/bin"
  action :nothing
end

version = node["ubik"]["golang"]["version"]
execute "install golang" do
  command "/usr/local/bin/godeb install #{version}"
  cwd "/var/cache/chef"
  not_if "go version 2>/dev/null | grep -q go#{version}"
end
