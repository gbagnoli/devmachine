# frozen_string_literal: true

# git-core is a virt pkg in bionic
pkgs = node["rbenv"]["install_pkgs"].map(&:dup).reject { |x| x == "git-core" }
pkgs << "git"
node.override["rbenv"]["install_pkgs"] = pkgs

# similarly, libgdm3 is not libgdm5
pkgs = node["ruby_build"]["install_pkgs_cruby"].map(&:dup).reject { |x| ["libgdbm3", "libgdm5"].include?(x) }
pkgs << "libgdbm6"
node.override["ruby_build"]["install_pkgs_cruby"] = pkgs

include_recipe "ruby_build"
include_recipe "ruby_rbenv::user"

user = "giacomo"
chef_workstation_dir = "/home/#{user}/.rbenv/versions/chef-workstation"
directory chef_workstation_dir do
  recursive true
  owner user
end
