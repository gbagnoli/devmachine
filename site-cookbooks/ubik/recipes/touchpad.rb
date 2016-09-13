
package 'xserver-xorg-input-mtrack'

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
        Option "ClickFinger3" "3"
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
        Option "TapButton2" "0"
        Option "TapButton3" "0"
        Option "TapButton4" "0"
        Option "ThumbSize" "35"
EndSection
  EOH
  mode '0644'
end
