include_recipe 'sudo'

group 'sysadmin' do
  gid 4000
end

sudo 'sysadmin' do
  group 'sysadmin'
end

node['bender']['users'].each do |username, user_details|
  user username do
    manage_home true
    uid user_details['uid']
    gid 'sysadmin'
    home "/home/#{username}"
    shell user_details['shell']
  end

  next if user_details['ssh_keys'].empty?

  directory "/home/#{username}/.ssh"
  file "/home/#{username}/.ssh/authorized_keys" do
    content user_details['ssh_keys'].join("\n")
    owner user_details['uid']
    group user_details['uid']
    action :create
  end
end
