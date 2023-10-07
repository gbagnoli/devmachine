local_file = "#{Chef::Config[:file_cache_path]}/mullvad.deb"

remote_file local_file do
  source 'https://mullvad.net/download/app/deb/latest'
end

dpkg_package "mullvad" do
  source local_file
end
