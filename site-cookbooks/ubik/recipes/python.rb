# frozen_string_literal: true

# git-core is a virt pkg in bionic and above
pkgs = node["pyenv"]["install_pkgs"].map(&:dup).reject { |x| x == "git-core" }
pkgs << "git"
node.override["pyenv"]["install_pkgs"] = pkgs

# looks like pyenv want /usr/bin/python to exist to use the system version
if node["lsb"]["codename"] == "focal"
  link '/usr/bin/python' do
    to '/usr/bin/python3'
  end
end

include_recipe "pyenv::user"
node["pyenv"]["user_installs"].each do |desc|
  desc["pythons"].each do |ver|
    pyenv_script "pyenv_install_bin_#{ver}_#{desc["user"]}" do
      user desc["user"]
      pyenv_version ver
      code "pip install -U -q pip pipenv"
    end
  end
end
