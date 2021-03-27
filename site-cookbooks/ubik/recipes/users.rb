# frozen_string_literal: true

include_recipe "user"
include_recipe "user::photos"
include_recipe "ubik::irene"

users = [node["user"]["login"], "irene"]

%w[adm sudo cdrom dip plugdev lpadmin sambashare users games lp].each do |grp|
  group grp do
    members users
    action :manage
  end
end

directory "/var/lib/steam" do
  user "root"
  group "games"
  mode "2775"
end

execute "setfacl_/var/lib/steam" do
  command "setfacl -R -d -m u::rwx g::rwx -m o::rx -m m::rwx /var/lib/steam"
  user "root"
  not_if "getfacl /var/lib/steam 2>/dev/null | grep 'default:group::rwx' -q"
end
