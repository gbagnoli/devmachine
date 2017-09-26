nerdfonts = '/usr/src/nerdfonts'
nerdfonts_repo = 'https://github.com/ryanoasis/nerd-fonts.git'

git nerdfonts do
  repository nerdfonts_repo
  revision 'master'
  action :sync
  user 'root'
  notifies :run, 'execute[install_nerdfonts]', :immediately
end

execute 'install_nerdfonts' do
  action :nothing
  command './install.sh -l -S -A'
  cwd nerdfonts
  user 'root'
end
