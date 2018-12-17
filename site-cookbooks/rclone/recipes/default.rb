# frozen_string_literal: true

deb = "#{Chef::Config[:file_cache_path]}/rclone.deb"

remote_file deb do
  source 'https://downloads.rclone.org/rclone-current-linux-amd64.deb'
  notifies :run, 'execute[install rclone]', :immediately
end

execute 'install rclone' do
  command "dpkg -i #{deb}"
  action :nothing
end
