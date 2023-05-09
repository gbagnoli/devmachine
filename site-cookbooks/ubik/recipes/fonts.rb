# frozen_string_literal: true

return unless node["ubik"]["install_fonts"]

nerdfonts = "/usr/src/nerdfonts"
nerdfonts_repo = "https://github.com/ryanoasis/nerd-fonts.git"

git nerdfonts do
  repository nerdfonts_repo
  depth 1
  revision "master"
  action :sync
  user "root"
  notifies :run, "execute[install_nerdfonts]", :immediately
end

execute "install_nerdfonts" do
  action :nothing
  command "./install.sh -l -S || exit 0"
  cwd nerdfonts
  user "root"
  ignore_failure true
end
