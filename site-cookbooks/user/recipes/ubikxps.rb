gnome_autostart 'Fix Touchpad' do
  script_content <<-HEREDOC
echo 'sleeping 10 seconds'
sleep 10s
synclient TapButton3=2
HEREDOC
  comment 'Fix touchpad config'
  user node['user']['login']
  action :install
end

file '/etc/sudoers.d/giacomo_syncthing' do
  mode '0440'
  content <<-HEREDOC
#{node['user']['login']} ALL=NOPASSWD: /bin/systemctl restart syncthing@giacomo
HEREDOC
end

gnome_autostart 'Restart syncthing' do
  script_content <<-HEREDOC
echo 'Sleeping 10 seconds'
sleep 10
echo 'Restarting syncthing'
sudo /bin/systemctl restart syncthing@giacomo
HEREDOC
  user node['user']['login']
  comment 'Restart syncthing to account for ecryptfs'
  action :install
end
