# frozen_string_literal: true

apt_repository 'profile_sync_daemon' do
  uri 'ppa:graysky/utils'
end

package 'profile-cleaner'
package 'profile-sync-daemon'

node['profile_sync_daemon']['users'].each do |user|
  if node['profile_sync_daemon']['overlayfs']
    file "/etc/sudoers.d/psd-#{user}" do
      content <<~EOH
        #{user} ALL=(ALL) NOPASSWD: /usr/bin/psd-overlay-helper
      EOH
    end
  end

  directory "/home/#{user}/.config/psd" do
    action :create
    owner user
    group user
    recursive true
  end

  template "/home/#{user}/.config/psd/psd.conf" do
    source 'psd.conf.erb'
    variables(
      overlayfs: node['profile_sync_daemon']['overlayfs'],
      browsers: node['profile_sync_daemon']['browsers'],
      backups: node['profile_sync_daemon']['backups']
    )
    owner user
    group user
  end

  directory "/home/#{user}/.config/systemd/user/default.target.wants/" do
    recursive true
    owner user
    group user
    notifies :run, 'execute[systemd_user_fix_perms]', :immediately
  end

  execute 'systemd_user_fix_perms' do
    action :nothing
    command "chown -R #{user} /home/#{user}/.config/systemd/"
  end

  link "/home/#{user}/.config/systemd/user/default.target.wants/psd.service" do
    to '/usr/lib/systemd/user/psd.service'
    owner user
    group user
  end
end
