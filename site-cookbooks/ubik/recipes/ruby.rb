# frozen_string_literal: true

if node['lsb']['codename'] == 'bionic'
  # git-core is a virt pkg in bionic
  pkgs = node['rbenv']['install_pkgs']\
         .map(&:dup).reject { |x| x == 'git-core' }
  pkgs << 'git'
  node.override['rbenv']['install_pkgs'] = pkgs

  # similarly, libgdm3 is not libgdm5
  pkgs = node['ruby_build']['install_pkgs_cruby']\
         .map(&:dup).reject { |x| x == 'libgdbm3' }
  pkgs << 'libgdbm5'
  node.override['ruby_build']['install_pkgs_cruby'] = pkgs
end

include_recipe 'ruby_build'
include_recipe 'ruby_rbenv::user'

user = 'giacomo'
chefdk_dir = "/home/#{user}/.rbenv/versions/chefdk"
directory chefdk_dir do
  recursive true
  owner user
end
