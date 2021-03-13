# frozen_string_literal: true

packages = []

package "language-selector-common"

[*node["ubik"]["languages"]].each do |lang|
  packages += ["language-pack-#{lang}",
               "language-pack-gnome-#{lang}",
               "language-pack-#{lang}-base",
               "language-pack-gnome-#{lang}-base"]
  begin
    clsupport = Mixlib::ShellOut.new("check-language-support -l #{lang}")
    clsupport.run_command
    clsupport.error!
    packages += clsupport.stdout.split
  rescue StandardError
  end
end

packages.each do |pkg|
  package pkg
end
