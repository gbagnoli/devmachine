directory '/var/cache/chef' do
  recursive true
end

remote_file '/var/cache/chef/godeb-amd64.tar.gz' do
  source 'https://godeb.s3.amazonaws.com/godeb-amd64.tar.gz'
  checksum 'efacfd01ad34f925460fe52e817005b6faa749442c851bd5bfac8d84c72ff261'
  notifies :run, 'execute[unpack godeb]', :immediately
end

execute 'unpack godeb' do
  command 'tar xvf /var/cache/chef/godeb-amd64.tar.gz -C /usr/local/bin'
  action :nothing
end

version = node['ubik']['golang']['version']
execute 'install golang' do
  command "/usr/local/bin/godeb install #{version}"
  cwd '/var/cache/chef'
  not_if "go version 2>/dev/null | grep -q go#{version}"
end



