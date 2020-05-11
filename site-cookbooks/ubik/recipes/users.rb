# frozen_string_literal: true

include_recipe "facl"
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

facl "/var/lib/steam" do
  user    '': 'rwx'
  group   '': 'rwx'
  mask    '': 'rwx'
  other   '': 'rx'
  default(
    user: { '': 'rwx' },
    group: { '':  'rwx' },
    mask: { '': 'rwx' },
    other: { '': 'rx' },
  )
end
