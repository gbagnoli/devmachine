source 'https://supermarket.getchef.com'

cookbook 'debconf', git: 'https://github.com/ophymx/debconf.git'
cookbook 'pleaserun', git: 'https://github.com/mjuarez/chef-pleaserun.git'
cookbook 'plex', git: 'https://github.com/gbagnoli/plex', branch: 'new_release_url'
cookbook 'ruby_rbenv', git: 'https://github.com/gbagnoli/ruby_rbenv', branch: 'ubuntu_20.04'
cookbook 'oauth2_proxy', git: 'https://github.com/gbagnoli/cookbook-oauth2_proxy'
cookbook 'seven_zip', '<=3.2.2'
cookbook 'pyenv', git: 'https://github.com/gbagnoli/pyenv', branch: 'ubuntu-jammy'

cbs = Dir.entries('site-cookbooks').select do |e|
  dir = File.join('site-cookbooks', e)
  File.directory?(dir) && !e.start_with?('.')
end

cbs.each do |cb|
  cookbook File.basename(cb), path: File.join('site-cookbooks', cb)
end
