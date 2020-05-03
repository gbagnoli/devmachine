# frozen_string_literal: true

# # git-core is a virt pkg in bionic
# pkgs = node["rbenv"]["install_pkgs"].map(&:dup).reject { |x| x == "git-core" }
# pkgs << "git"
# node.override["rbenv"]["install_pkgs"] = pkgs

# # similarly, libgdm3 is not libgdm5
# pkgs = node["ruby_build"]["install_pkgs_cruby"].map(&:dup).reject { |x| ["libgdbm3", "libgdm5"].include?(x) }
# pkgs << "libgdbm6"
# node.override["ruby_build"]["install_pkgs_cruby"] = pkgs

user = node["ubik"]["ruby"]["user"]
rbenv_user_install user

rbenv_plugin 'ruby-build' do
  git_url 'https://github.com/rbenv/ruby-build.git'
  user user
end

rbenv_plugin 'chefdk' do
  git_url 'https://github.com/docwhat/rbenv-chefdk.git'
  user user
end

node["ubik"]["ruby"]["rubies"]&.each do |ruby|
  rbenv_ruby ruby do
    user user
  end

  %w[bundler rubocop].each do |g|
    rbenv_gem g do
      user user
      rbenv_version ruby
    end
  end
end

chef_workstation_dir = "/home/#{user}/.rbenv/versions/chef-workstation"
directory chef_workstation_dir do
  recursive true
  owner user
end
