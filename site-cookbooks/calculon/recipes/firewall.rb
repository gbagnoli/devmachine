package "firewalld"

execute "persist_firewalld" do
  command "firewall-cmd --runtime-to-permanent"
  action :nothing
end
