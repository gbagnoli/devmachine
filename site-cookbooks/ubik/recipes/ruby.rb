# frozen_string_literal: true

include_recipe 'ruby_rbenv::user'

user = 'giacomo'
chefdk_dir = "/home/#{user}/.rbenv/versions/chefdk"
directory chefdk_dir do
  recursive true
  owner user
end
