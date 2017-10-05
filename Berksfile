source "https://supermarket.getchef.com"

cookbook 'pyenv', git: 'https://github.com/sds/chef-pyenv.git'
cookbook 'debconf', git: 'https://github.com/ophymx/debconf.git'
cookbook 'ssh-hardening'

cbs = Dir.entries('site-cookbooks').select do |e|
  dir = File.join('site-cookbooks', e)
  File.directory?(dir) && !e.start_with?('.')
end

cbs.each do |cb|
  cookbook File.basename(cb), path: File.join('site-cookbooks', cb)
end
