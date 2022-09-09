package "python3-rpi.gpio"
package "python3-smbus"

template "/etc/argononed.conf" do
  source "argononed.conf.erb"
  variables fan_control: node["argonone"]["fan_control"]
  mode '644'
  owner "root"
  group "root"
end

cookbook_file "/lib/systemd/system-shutdown/argononed-poweroff.py" do
  source "argononed-poweroff.py"
  owner "root"
  group "root"
  mode '755'
end

cookbook_file "/usr/bin/argononed.py" do
  source "argononed.py"
  owner "root"
  group "root"
  mode '755'
end

cookbook_file "/usr/bin/argon_temp_monitor" do
  source "argon_temp_monitor.sh"
  mode '755'
  owner "root"
  group "root"
end

systemd_unit "argononed.service" do
  content <<~EOU
    [Unit]
    Description=Argon One Fan and Button Service
    After=multi-user.target
    [Service]
    Type=simple
    Restart=always
    RemainAfterExit=true
    ExecStart=/usr/bin/python3 /usr/bin/argononed.py
    [Install]
    WantedBy=multi-user.target
  EOU
  action %i(create enable start)
end
