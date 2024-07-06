# frozen_string_literal: true

user = node["ubik"]["ruby"]["user"]
rbenv_user_install user

rbenv_plugin 'ruby-build' do
  git_url 'https://github.com/rbenv/ruby-build.git'
  user user
end

rbenv_plugin 'chef-workstation' do
  git_url 'https://github.com/docwhat/rbenv-chef-workstation.git'
  user user
  only_if { ::File.directory?("/opt/chef-workstation") }
end

rbenv_plugin 'cinc-workstation' do
  git_url 'https://github.com/david-alpert-nl/rbenv-chef-workstation.git'
  git_ref "cinc-workstation"
  user user
  only_if { ::File.directory?("/opt/cinc-workstation") }
end

node["ubik"]["ruby"]["rubies"]&.each do |ruby|
  rbenv_ruby ruby do
    user user
  end

  %w(bundler rubocop).each do |g|
    rbenv_gem g do
      user user
      rbenv_version ruby
    end
  end
end

chefworkstation_dir = "/home/#{user}/.rbenv/versions/chef-workstation"
directory chefworkstation_dir do
  recursive true
  owner user
  only_if { ::File.directory?("/opt/chef-workstation") }
end

cincworkstation_dir = "/home/#{user}/.rbenv/versions/cinc-workstation"
directory cincworkstation_dir do
  recursive true
  owner user
  only_if { ::File.directory?("/opt/cinc-workstation") }
end
