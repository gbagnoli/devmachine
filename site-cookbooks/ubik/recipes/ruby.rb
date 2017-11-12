include_recipe 'ruby_rbenv::user'

user = "giacomo"
chefdk_dir = "/home/#{user}/.rbenv/versions/chefdk"
directory chefdk_dir

rbenv_script "finish-rbenv-chefdk-plugin-install" do
  rbenv_version "chefdk"
  user user
  code %{rbenv rehash}
  subscribes :run, "directory[#{chefdk_dir}]", :immediately
  action :nothing
end
