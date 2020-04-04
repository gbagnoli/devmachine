if node["lsb"]["codename"] == "xenial"
  package "xserver-xorg-input-libinput-hwe-16.04"
else
  package "xserver-xorg-input-libinput"
end

x11_conf_d = "/usr/share/X11/xorg.conf.d/"

file "#{x11_conf_d}/51-mtrack.conf" do
  action :delete
end

apt_package "xserver-xorg-input-mtrack" do
  action :purge
end

file "#{x11_conf_d}/50-xps-touchpad.conf" do
  mode "0644"
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
      Identifier "XPS touchpad"
      Driver "libinput"
      MatchIsTouchpad "true"
      Option "Tapping" "True"
      Option "TappingDragLock" "True"
      Option "PalmDetection" "True"
      Option "ButtonMapping" "1 3 2"
      Option "DisableWhileTyping" "True"
      Option "NaturalScrolling" "True"
      Option "ScrollMethod" "twofinger"
      Option "HorizontalScrolling" "True"
      # Option "AccelProfile" "adaptive"
      # Option "AccelSpeed" "0.1"
      # Option "ClickMethod" "clickfinger"
      # Option "MiddleEmulation" "True"
      Option "SendEventsMode" "disabled-on-external-mouse"
    EndSection
          EOH
end
