include_recipe 'sudo'

node['server']['users'].each do |username, user_details|
  next if user_details['unmanaged'] && !user_details['delete']

  user_action = :create
  group_action = :create
  callback = :nothing
  if user_details['delete']
    user_action = :remove
    group_action = :nothing
    callback = :remove
  end

  group username do
    gid user_details['uid']
    action group_action
  end

  user username do
    manage_home true
    uid user_details['uid']
    gid username
    home "/home/#{username}"
    shell user_details['shell']
    action user_action
    notifies callback, "group[#{username}]", :immediately
  end

  next if user_details['delete'] || user_details['ssh_keys'].to_a.empty?

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

sysadmins = node['server']['users'].reject { |_, v| v['unmanaged'] || v['delete'] }.keys.dup
sysadmins << node['user']['login'] if node['server']['components']['user']['enabled']

group 'sysadmins' do
  gid 3000
  members sysadmins.sort
end

sudo 'sysadmins' do
  group 'sysadmins'
end
