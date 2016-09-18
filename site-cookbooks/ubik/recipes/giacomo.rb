user = 'giacomo'
group = 'giacomo'
home = "/home/#{user}"
uid = 1000
gid = 1000
realname = 'Giacomo Bagnoli'

user user do
  group group
  shell '/bin/bash'
  uid uid
  gid gid
  home home
  comment realname
end

["#{home}/.local",
 "#{home}/.local/src",
 "#{home}/.config",
 "#{home}/.config/nvim",
 "#{home}/.config/nvim/bundle",
 "#{home}/workspace",
 "#{home}/workspace/go"].each do |d|
  directory d do
    mode '0750'
    owner user
    group user
  end
end

dotfiles = "#{home}/.local/src/dotfiles"
git dotfiles do
  repository 'https://github.com/gbagnoli/dotfiles.git'
  revision 'master'
  action :sync
  user user
end

link "#{home}/.inputrc" do
  to "#{dotfiles}/inputrc"
end

link "#{home}/.gitconfig" do
  to "#{dotfiles}/gitconfig"
end

link "#{home}/.gitignore_global" do
  to "#{dotfiles}/gitignore"
end

link "#{home}/.tmux.conf" do
  to "#{dotfiles}/tmux.conf"
end

link "#{home}/.vim" do
  to "#{home}/.config/nvim"
end

link "#{home}/.config/nvim/init.vim" do
  to "#{dotfiles}/vim/vimrc"
end

%w(vim-nox python-dev python-pip python3-dev python3-pip).each do |pkg|
  package pkg
end

package 'neovim' do
  action :install
  notifies :run, 'bash[set nvim alternatives]', :immediately
end

bash 'set nvim alternatives' do
  code <<-EOH
  update-alternatives --install /usr/bin/vi vi /usr/bin/nvim 60
  update-alternatives --config vi
  update-alternatives --install /usr/bin/vim vim /usr/bin/nvim 60
  update-alternatives --config vim
  update-alternatives --install /usr/bin/editor editor /usr/bin/nvim 60
  update-alternatives --config editor
  EOH
  action :nothing
end

git "#{home}/.config/nvim/bundle/Vundle.vim" do
  repository 'https://github.com/VundleVim/Vundle.vim.git'
  action :sync
  notifies :run, 'bash[install vundle]', :immediately
  user user
end

bash 'install vundle' do
  action :nothing
  user user
  cwd home
  code <<-EOH
    nvim +PluginInstall +qall!
  EOH
end

git "#{home}/.local/src/vpnutils" do
  repository "git@bitbucket.org:gbagnoli/vpnutils.git"
  action :sync
  revision 'development'
  user user
end

git "#{home}/.local/src/autoenv" do
  repository 'git://github.com/kennethreitz/autoenv.git'
  action :sync
  user user
end

package 'liquidprompt'
package 'autossh'
package 'mosh'
package 'fasd'

cookbook_file "#{home}/.config/liquid.theme" do
  source 'liquid.theme'
  mode '0640'
  owner user
  group group
end

template "#{home}/.config/liquidpromptrc" do
  owner user
  group group
  mode '0640'
  source 'liquidpromptrc.erb'
end

template "#{home}/.bashrc" do
  owner user
  group group
  mode '0640'
  source 'bashrc.erb'
end

if File.directory? "#{home}/Sync/Private/weechat"
  link "#{home}/.weechat" do
    to "#{home}/Sync/Private/weechat"
  end
end

file "#{home}/.bashrc.local" do
  action :create_if_missing
end
