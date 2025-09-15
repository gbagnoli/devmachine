# frozen_string_literal: true

default["ubik"]["golang"]["version"] = "1.15.3"
default["ubik"]["skip_packages"] = false
default["ubik"]["install_latex"] = false
default["ubik"]["install_fonts"] = false
default["ubik"]["ruby"]["user"] = "giacomo"
default["ubik"]["ruby"]["rubies"] = []

default["ubik"]["rust"]["user"] = "giacomo"
default["ubik"]["rust"]["version"] = "stable"

default["usb"]["always_on_devices"] = {}
