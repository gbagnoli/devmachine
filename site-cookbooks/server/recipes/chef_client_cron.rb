# this will not update code, only run chef
#
directory "/var/log/chef/"

cron "chef-client" do
  action :create
  minute node["server"]["chef"]["cron"]["minute"]
  hour node["server"]["chef"]["cron"]["hour"]
  user "root"
  home "/usr/local/src/chefrepo/"
  path "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/snap/bin"
  command "/usr/local/bin/run-chef -l info -L /var/log/chef/client.log &>/dev/null"
end

logrotate_app "chef-client" do
  path "/var/log/chef/client.log"
  frequency "daily"
  rotate 30
  create "644 root root"
end
