include_recipe 'sudo'

node['server']['users'].each do |username, user_details|
  next if user_details['unmanaged']

  group username do
    gid user_details['uid']
  end

  user username do
    manage_home true
    uid user_details['uid']
    gid username
    home "/home/#{username}"
    shell user_details['shell']
  end

  next if user_details['ssh_keys'].empty?

  directory "/home/#{username}/.ssh" do
    owner user_details['uid']
    group user_details['uid']
  end

  file "/home/#{username}/.ssh/authorized_keys" do
    content user_details['ssh_keys'].join("\n")
    owner user_details['uid']
    group user_details['uid']
  end
end

sysadmins = node['server']['users'].reject { |_, v| v['unmanaged'] }.keys.dup
if node['server']['components']['user']['enabled']
  sysadmins << node['user']['login']
end

group 'sysadmins' do
  gid 3000
  members sysadmins.sort
end

sudo 'sysadmins' do
  group 'sysadmins'
end
