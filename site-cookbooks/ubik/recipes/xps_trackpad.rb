
x11_conf_d = '/usr/share/X11/xorg.conf.d/'

file "#{x11_conf_d}/51-mtrack.conf" do
  action :delete
end

apt_package 'xserver-xorg-input-mtrack' do
  action :purge
end

file "#{x11_conf_d}/50-xps-touchpad.conf" do
  content <<~EOH
    # Disable generic Synaptics device, as we're using
    # "DLL0704:01 06CB:76AE Touchpad"
    # Having multiple touchpad devices running confuses syndaemon
    Section "InputClass"
      Identifier "SynPS/2 Synaptics TouchPad"
      MatchProduct "SynPS/2 Synaptics TouchPad"
      MatchIsTouchpad "on"
      MatchOS "Linux"
      MatchDevicePath "/dev/input/event*"
      Option "Ignore" "on"
    EndSection
     Section "InputClass"
      Identifier "touchpad catchall"
      Driver "synaptics"
      MatchIsTouchpad "on"
      Option "PalmDetect" "1"
      Option "PalmMinWidth" "5"
      Option "PalmMinZ" "5"
      # This option is recommend on all Linux systems using evdev, but cannot be
      # enabled by default. See the following link for details:
      # http://who-t.blogspot.com/2010/11/how-to-ignore-configuration-errors.html
      MatchDevicePath "/dev/input/event*"
    EndSection
EOH
end
