source "https://supermarket.getchef.com"

cookbook 'pyenv', git: 'https://github.com/sds/chef-pyenv.git'

cbs = Dir.entries('site-cookbooks').select do |e|
  dir = File.join('site-cookbooks', e)
  File.directory?(dir) && !e.start_with?('.')
end

cbs.each do |cb|
  cookbook File.basename(cb), path: File.join('site-cookbooks', cb)
end

# cookbook 'ubik', path: 'site-cookbooks/ubik'
# cookbook 'upik', path: 'site-cookbooks/upik'
# cookbook 'syncthing', path: 'site-cookbooks/upik'
