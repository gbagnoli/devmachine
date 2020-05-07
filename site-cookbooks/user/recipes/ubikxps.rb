sudo "#{node["user"]["login"]}_syncthing" do
  commands ["/bin/systemctl restart syncthing@#{node["user"]["login"]}"]
  nopasswd true
  user node["user"]["login"]
end

gnome_autostart "Restart syncthing" do
  script_content <<~HEREDOC
    echo 'Sleeping 10 seconds'
    sleep 10
    echo 'Restarting syncthing'
    sudo /bin/systemctl restart syncthing@giacomo
  HEREDOC
  user node["user"]["login"]
  comment "Restart syncthing to account for ecryptfs"
  action :install
end
