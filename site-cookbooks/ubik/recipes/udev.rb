path = "/etc/udev/rules.d/70-usb-power.rules"

unless node["usb"]["always_on_devices"].empty?
  template path do
    source "udev-usb-power.rules.erb"
    mode "0644"
    owner "root"
    variables always_on_devices: node["usb"]["always_on_devices"]
  end
end
