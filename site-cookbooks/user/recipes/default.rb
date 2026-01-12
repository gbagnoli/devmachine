# frozen_string_literal: true

user = node["user"]["login"]
group = node["user"]["group"]
home = "#{node["user"]["homedir"]}/#{user}"
gid = node["user"]["gid"]

group user do
  gid gid
end

uid = node["user"]["uid"]
realname = node["user"]["realname"]
user user do
  group group
  shell "/bin/bash"
  manage_home true
  uid uid
  gid user
  home home
  comment realname
  not_if "getent passwd #{user}"
end

[home, "#{home}/.ssh"].each do |d|
  directory d do
    action :create
    owner user
    group group
    mode "0700"
  end
end

["#{home}/.local",
 "#{home}/.local/bin",
 "#{home}/.local/src",
 "#{home}/.config",
 "#{home}/.config/nvim",
 "#{home}/.config/nvim/bundle",
 "#{home}/workspace",
 "#{home}/workspace/go"].each do |d|
  directory d do
    mode "0750"
    owner user
    group group
  end
end

package "git"
dotfiles = "#{home}/.local/src/dotfiles"
git dotfiles do
  repository "https://github.com/gbagnoli/dotfiles.git"
  revision "master"
  enable_checkout false
  checkout_branch "master"
  action :sync
  user user
  notifies :run, "execute[install dotfiles]", :immediately
end

execute "install dotfiles" do
  action :run
  environment HOME: home
  cwd dotfiles
  command "#{dotfiles}/install.sh"
  user user
  group group
end

%w(inputrc gitconfig bashrc gitignore_global tmux.conf profile).each do |conf|
  path="#{home}/.#{conf}"
  file path do
    action :delete
    not_if { File.symlink? path }
  end

  link path do
    to "#{dotfiles}/#{conf}"
  end
end

{ '.config/flake8': "#{dotfiles}/flake8",
  '.config/liquidpromptrc': "#{dotfiles}/liquidpromptrc",
  '.config/nvim/init.vim': "#{dotfiles}/vim/vimrc",
  '.vim': "#{home}/.config/nvim",
  '.vimrc': "#{dotfiles}/vim/vimrc" }.each do |source, dest|
  file "#{home}/#{source}" do
    action :delete
    not_if { File.symlink? path }
  end

  link "#{home}/#{source}" do
    to dest
  end
end

if platform?("debian")
  package "libc6-dev"
  package "libexpat1-dev"
  package "libpython2.7-dev"
end

packages = value_for_platform(
  %w{ubuntu debian} => {default: %w{python3-dev vim vim-nox python3-pip}},
  %w{centos fedora rocky} => {default: %w{python3-devel vim python3-pip}},
)

package "editors" do
  package_name packages
  action :install
end

package "neovim" do
  action :install
  notifies :run, "bash[set vim alternatives]", :immediately
  only_if { node.platform_family?("debian") }
end


bash "set vim alternatives" do
  code <<-EOH
  update-alternatives --install /usr/bin/vi vi /usr/bin/nvim 35
  update-alternatives --config vi
  update-alternatives --install /usr/bin/vim vim /usr/bin/nvim 35
  update-alternatives --config vim
  update-alternatives --install /usr/bin/editor editor /usr/bin/nvim 35
  update-alternatives --config editor
  EOH
  action :nothing
end

package "sudo"

git "#{home}/.config/nvim/bundle/Vundle.vim" do
  repository "https://github.com/VundleVim/Vundle.vim.git"
  action :sync
  notifies :run, "bash[install vundle]", :immediately
  user user
end

bash "install vundle" do
  action :nothing
  cwd home
  user user
  code <<-EOH
     echo | echo | vim +PluginInstall! +qall &>/dev/null
  EOH
end

%w(bitbucket.org github.com).each do |site|
  ssh_known_hosts_entry site
end

git "#{home}/.local/src/autoenv" do
  repository "https://github.com/hyperupcall/autoenv.git"
  action :sync
  user user
end

git "/usr/share/liquidprompt" do
  repository "https://github.com/liquidprompt/liquidprompt.git"
  action :sync
  user "root"
  revision "master"
end

file "/etc/liquidpromptrc" do
  action :delete
end

file "#{home}/.bashrc.local" do
  action :create_if_missing
end

group "docker" do
  action :manage
  members node["user"]["login"]
end

node["user"]["ssh_authorized_keys"].each do |desc|
  ssh_authorize_key desc[:name] do
    key desc[:pubkey]
    user node["user"]["login"]
    keytype desc[:keytype]
    not_if { File.symlink?("#{home}/.ssh/authorized_key") }
  end
end
