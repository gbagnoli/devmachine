# Cookbook:: flatpak
# Recipe:: default
#
# Copyright:: 2020, The Authors, All Rights Reserved.

package 'flatpak'
package 'gnome-software-plugin-flatpak'

execute 'add_flathub_repo' do
  action :nothing
  command 'flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo'
  subscribes :run, "package[flatpak]", :immediately
end
