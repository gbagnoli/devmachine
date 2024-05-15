file "/etc/modules-load.d/zram.conf" do
  content <<~EOH
    zram
  EOH
end

file "/etc/modprobe.d/zram.conf" do
  content <<~EOH
    options zram num_devices=1
  EOH
end

file "/etc/udev/rules.d/99-zram.rules" do
  content <<~EOH
    KERNEL=="zram0", ATTR{disksize}="6G",TAG+="systemd"
  EOH
end

systemd_unit "zram.service" do
  content <<~EOH
    [Unit]
    Description=Swap with zram
    After=multi-user.target

    [Service]
    Type=oneshot
    RemainAfterExit=true
    ExecStartPre=/sbin/mkswap /dev/zram0
    ExecStart=/sbin/swapon -p 10 /dev/zram0
    ExecStop=/sbin/swapoff /dev/zram0

    [Install]
    WantedBy=multi-user.target
  EOH
  action %i(create enable start)
end
