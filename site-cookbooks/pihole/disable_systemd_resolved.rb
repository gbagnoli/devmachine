# disable stub DNS resolver for systemd-resolve
file "/etc/systemd/resolved.conf" do
  content <<~EOU
    [Resolve]
     DNSStubListener=no
  EOU
  notifies :restart, "service[systemd-resolved]", :immediately
end

link "/etc/resolv.conf" do
  to "/run/systemd/resolve/resolv.conf"
  notifies :restart, "service[systemd-resolved]", :immediately
end

service "systemd-resolved" do
  action %i(nothing)
end
