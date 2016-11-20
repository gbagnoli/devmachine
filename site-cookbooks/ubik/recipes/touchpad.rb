version = "0.4.1"
package_name = "xserver-xorg-input-mtrack"
deb = "#{package_name}_#{version}_amd64.deb"
repo = "https://github.com/p2rkw/xf86-input-mtrack/"

# install the "old" version
package package_name

remote_file "#{Chef::Config[:file_cache_path]}/#{deb}" do
  source "#{repo}/releases/download/v#{version}/#{deb}"
  action :create
  checksum '0c7ba857b55002bc02ee27e6854c09844d05a084b4cfac78b0d7ff120a9f6494'
  notifies :run, 'execute[install_mtrack_deb]', :immediately
end

execute 'install_mtrack_deb' do
  action :nothing
  command "dpkg -i #{Chef::Config[:file_cache_path]}/#{deb}"
end

file '/usr/share/X11/xorg.conf.d/51-mtrack.conf' do
  content <<-EOH
Section "InputClass"
        MatchIsTouchpad "on"
        Identifier      "Touchpads"
        Driver          "mtrack"
        # Option "AccelerationProfile" "2"
        Option "AdaptiveDeceleration" "2.0" # Decelerate slow movements
        Option "BottomEdge" "40"
        Option "ButtonIntegrated" "true"
        Option "ButtonMoveEmulate" "true"
        Option "ClickFinger1" "1"
        Option "ClickFinger2" "3"
        Option "ClickFinger3" "2"
        Option "ClickTime" "25"
        Option "ConstantDeceleration" "2.0" # Decelerate endspeed
        Option "DisableOnPalm" "true"
        Option "DisableOnThumb" "false"
        Option "FingerHigh" "12"
        Option "FingerLow" "4"
        Option "IgnorePalm" "true"
        Option "IgnoreThumb" "true"
        Option "PalmSize" "55"
        Option "ScrollDistance" "75"
        Option "ScrollDownButton" "5"
        Option "ScrollLeftButton" "7"
        Option "ScrollRightButton" "6"
        Option "ScrollUpButton" "4"
        Option "Sensitivity" "0.65"
        Option "SwipeDistance" "1000"
        Option "SwipeDownButton" "0"
        Option "SwipeLeftButton" "8"
        Option "SwipeRightButton" "9"
        Option "SwipeUpButton" "0"
        Option "TapButton1" "1"
        Option "TapButton2" "3"
        Option "TapButton3" "2"
        Option "TapButton4" "0"
        Option "ThumbSize" "35"
EndSection
  EOH
  mode '0644'
end
