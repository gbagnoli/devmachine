package 'perl'
package 'btrfs-progs'

git node['btrbk']['src_dir'] do
  repository node['btrbk']['repository']
  revision node['btrbk']['revision']
  action :sync
  notifies :run, 'execute[install btrbk]', :immediately
end

execute 'install btrbk' do
  action :nothing
  command 'make install'
  cwd node['btrbk']['src_dir']
end
