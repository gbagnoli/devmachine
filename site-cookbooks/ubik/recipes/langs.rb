packages = []

[*node['ubik']['languages']].each do |lang|
  packages += ["language-pack-#{lang}",
               "language-pack-gnome-#{lang}",
               "language-pack-#{lang}-base",
               "language-pack-gnome-#{lang}-base"]
  packages += `check-language-support -l #{lang}`.split(' ')
end

packages.each do |pkg|
  package pkg
end

