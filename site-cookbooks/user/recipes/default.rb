# frozen_string_literal: true

user = node["user"]["login"]
group = node["user"]["group"]
home = "/home/#{user}"
uid = node["user"]["uid"]
gid = node["user"]["gid"]
realname = node["user"]["realname"]

group user do
  gid gid
end

user user do
  group group
  shell "/bin/bash"
  manage_home true
  uid uid
  gid user
  home home
  comment realname
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

link "#{home}/.vimrc" do
  to "#{dotfiles}/vim/vimrc"
end

if platform?("debian")
  package "libc6-dev"
  package "libexpat1-dev"
  package "libpython2.7-dev"
end

packages = value_for_platform(
  %w{ubuntu debian} => {default: %w{python3-dev vim vim-nox python3-pip}},
  %w{centos fedora} => {default: %w{python3-devel vim neovim python3-pip}},
)

package "editors" do
  package_name packages
  action :install
end

package "nvim" do
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

if node["user"]["install_vpnutils"]
  git "#{home}/.local/src/vpnutils" do
    repository "git@github.com:gbagnoli/vpnutils.git"
    action :sync
    revision "development"
    user user
  end
end

git "#{home}/.local/src/autoenv" do
  repository "https://github.com/hyperupcall/autoenv.git"
  action :sync
  user user
end

if node.platform_family? "debian"
  package "liquidprompt"

  if platform?('debian')
    remote_file "/usr/bin/fasd" do
      source "https://raw.githubusercontent.com/clvv/fasd/master/fasd"
      mode "0755"
    end
  else
    codename = "focal" # no jammy yet
    apt_repository "fasd" do
      uri "ppa:aacebedo/fasd"
      distribution codename
    end

    package "fasd"
  end
elsif node.platform_family? "fedora"
  git "/usr/share/liquidprompt" do
    repository "https://github.com/nojhan/liquidprompt.git"
    action :sync
    user "root"
    revision "stable"
  end
end

cookbook_file "#{home}/.config/liquid.theme" do
  source "liquid.theme"
  mode "0640"
  owner user
  group group
end

template "#{home}/.config/liquidpromptrc" do
  owner user
  group group
  mode "0640"
  source "liquidpromptrc.erb"
end

template "#{home}/.bashrc" do
  owner user
  group group
  mode "0640"
  source "bashrc.erb"
end

cookbook_file "#{home}/.profile" do
  source "profile"
  mode "0640"
  owner user
  group group
end

file "#{home}/.bashrc.local" do
  action :create_if_missing
end

cookbook_file "#{home}/.config/flake8" do
  source "flake8"
  mode "0644"
  owner user
  group group
end

sudo "#{node["user"]["login"]}_docker" do
  nopasswd true
  commands ["/usr/bin/docker"]
  user node["user"]["login"]
  action :delete
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
  end
end
