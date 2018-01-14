directory '/var/cache/chef' do
  recursive true
end

remote_file '/var/cache/chef/godeb-amd64.tar.gz' do
  source 'https://godeb.s3.amazonaws.com/godeb-amd64.tar.gz'
  checksum '4e73d1621495cc2b909893b9d31d74caec9110c0000218d123d8515f87e9c3ff'
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



