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

git "/usr/share/liquidprompt" do
  repository "https://github.com/liquidprompt/liquidprompt.git"
  action :sync
  user "root"
  revision "master"
end

file "#{home}/.config/liquid.theme" do
  action :delete
end

file "#{home}/.config/liquidpromptrc" do
  action :delete
end

template "/etc/liquidpromptrc" do
  owner "root"
  group "root"
  mode "0755"
  source "liquidpromptrc.erb"
end

file "#{home}/.bashrc.local" do
  action :create_if_missing
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

machine = node["kernel"]["machine"]
ruby_block "get zoxide latest version" do
  block do
    uri = URI("https://api.github.com/repos/ajeetdsouza/zoxide/releases/latest")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)
    asset = parsed["assets"].select {|x| x["name"].include?("#{machine}-unknown-linux")}.first
    node.run_state["zoxide_download_url"] = asset["browser_download_url"]
    node.run_state["zoxide_version"] = parsed["tag_name"][1..]
  end
end

zoxide_tar = "#{Chef::Config[:file_cache_path]}/zoxide.latest.tar.gz"
remote_file zoxide_tar do
  source(lazy { node.run_state["zoxide_download_url"] })
  notifies :run, "bash[install_zoxide]", :immediately
end

bash "install_zoxide" do
  action :nothing
  code <<~EOH
    tar -xpzf #{zoxide_tar} -C /usr/bin zoxide
    tar --strip-components=1 -xpzf #{zoxide_tar} -C /etc/bash_completion.d/ completions/zoxide.bash
  EOH
end


arch = case machine
       when "x86_64"
         "amd64"
       when "aarch64"
         "arm64"
       else
         Chef::Log.fatal("Unsupported arch #{node["kernel"]["machine"]}")
         raise
       end

ruby_block "get fzf latest version" do
  block do
    uri = URI("https://api.github.com/repos/junegunn/fzf/releases/latest")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)
    asset = parsed["assets"].select {|x| x["name"].include?("linux_#{arch}")}.first
    node.run_state["fzf_download_url"] = asset["browser_download_url"]
    node.run_state["fzf_version"] = parsed["tag_name"][1..]
  end
end

fzf_tar = "#{Chef::Config[:file_cache_path]}/fzf.latest.tar.gz"
remote_file fzf_tar do
  source(lazy { node.run_state["fzf_download_url"] })
  notifies :run, "bash[install_fzf]", :immediately
end

bash "install_fzf" do
  action :nothing
  code <<~EOH
    tar -xpzf #{fzf_tar} -C /usr/bin fzf
  EOH
end
