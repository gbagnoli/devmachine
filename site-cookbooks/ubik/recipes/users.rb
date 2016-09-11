include_recipe 'ubik::giacomo'
include_recipe 'ubik::irene'

users = %w(giacomo irene)

%w(adm sudo cdrom dip plugdev lpadmin sambashare users).each do |grp|
  group grp do
    members users
    action :modify
  end
end
