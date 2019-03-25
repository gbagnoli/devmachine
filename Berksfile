source 'https://supermarket.getchef.com'

cookbook 'pyenv', git: 'https://github.com/sds/chef-pyenv.git', revision: '89600c35ea1b5f3de3c9330a80dc62d4021cfd17'
cookbook 'debconf', git: 'https://github.com/ophymx/debconf.git'
cookbook 'pleaserun', git: 'https://github.com/mjuarez/chef-pleaserun.git'
cookbook 'plex', git: 'https://github.com/gbagnoli/plex', branch: 'new_release_url'

cbs = Dir.entries('site-cookbooks').select do |e|
  dir = File.join('site-cookbooks', e)
  File.directory?(dir) && !e.start_with?('.')
end

cbs.each do |cb|
  cookbook File.basename(cb), path: File.join('site-cookbooks', cb)
end
