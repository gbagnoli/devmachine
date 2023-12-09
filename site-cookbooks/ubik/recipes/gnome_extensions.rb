gnome_extension "sound-output-device-chooser@kgshank.net" do
  repository "https://github.com/kgshank/gse-sound-output-device-chooser.git"
  install_script <<-EOH
  rm -rf %<install_dir>s
  cp -r %<src_dir>s/%<name>s %<install_dir>s
  EOH
end

gnome_extension "no-title-bar@jonaspoehler.de" do
  repository "https://github.com/poehlerj/no-title-bar.git"
  install_script <<-EOH
  DESTDIR=/ make install
  EOH
  deps %w(x11-utils)
end

gnome_extension "freon@UshakovVasilii_Github.yahoo.com" do
  repository "https://github.com/UshakovVasilii/gnome-shell-extension-freon.git"
  install_script <<-EOH
  glib-compile-schemas %<name>s
  rm -rf %<install_dir>s
  cp -r %<src_dir>s/%<name>s %<install_dir>s
  chmod o+rX -R %<install_dir>s
  EOH
  deps %w(nvme-cli)
end

gnome_extension "emoji-selector@maestroschan.fr" do
  repository "https://github.com/maoschanz/emoji-selector-for-gnome.git"
  install_script <<-EOH
  glib-compile-schemas %<name>s
  rm -rf %<install_dir>s
  cp -r %<src_dir>s/%<name>s %<install_dir>s
  chmod o+rX -R %<install_dir>s
  EOH
end

gnome_extension "tailscale-status@maxgallup.github.com" do
  repository "https://github.com/maxgallup/tailscale-status.git"
  revision "main"
  install_script <<-EOH
  rm -rf %<install_dir>s
  cp -r %<src_dir>s/%<name>s %<install_dir>s
  EOH
end
