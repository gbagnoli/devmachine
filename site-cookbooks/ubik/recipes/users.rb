include_recipe 'user'
include_recipe 'ubik::irene'

users = [node['user']['login'], 'irene']

%w(adm sudo cdrom dip plugdev lpadmin sambashare users lp).each do |grp|
  group grp do
    members users
    action :modify
  end
end
