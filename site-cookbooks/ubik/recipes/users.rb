# frozen_string_literal: true

include_recipe "user"
include_recipe "user::photos"
include_recipe "ubik::irene"

users = [node["user"]["login"], "irene"]

%w[adm sudo cdrom dip plugdev lpadmin sambashare users lp].each do |grp|
  group grp do
    members users
    action :manage
  end
end
