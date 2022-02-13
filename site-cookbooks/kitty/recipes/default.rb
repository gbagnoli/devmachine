include_recipe 'ark'

ruby_block "get kitty latest version" do
  block do
    uri = URI("https://api.github.com/repos/kovidgoyal/kitty/releases/latest")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)
    asset = parsed["assets"].select {|x| x["label"] == "Linux amd64 binary bundle"}.first
    node.run_state["kitty_download_url"] = asset["browser_download_url"]
    node.run_state["kitty_version"] = parsed["tag_name"][1..]
  end
end

ark 'kitty' do
  url(lazy { node.run_state["kitty_download_url"] })
  version(lazy { node.run_state["kitty_version"] })
  action :install
  backup false
  strip_components 0
end

directory "/usr/local/bin/" do
  mode 0o755
end

bin = "/usr/local/bin/kitty"
alt_name = "x-terminal-emulator"
alt_link = "/usr/bin/#{alt_name}"

link bin do
  to "/usr/local/kitty/bin/kitty"
  notifies :run, "execute[add kitty #{alt_name} alternative]", :immediately
end

execute "add kitty #{alt_name} alternative" do
  command "update-alternatives --install #{alt_link} #{alt_name} #{bin} 30"
  action :nothing
  notifies :run, "execute[use kitty as default #{alt_name}]", :immediately
end

execute "use kitty as default #{alt_name}" do
  command "update-alternatives --set #{alt_name} #{bin}"
  action :nothing
  not_if node["kitty"]["set-alternative"] == false
end

node["kitty"]["users"].to_a.each do |info|
  gnome_desktop_file do
    name "kitty"
    user info["user"]
    group info["group"]
    exec "/usr/local/bin/kitty"
    type "Application"
    options(
      "Icon" => "/usr/local/kitty/share/icons/hicolor/256x256/apps/kitty.png",
      "Categories" => "System;TerminalEmulator;",
      "GenericName" => "Terminal emulator",
      "Version" => "1.0",
      "Comment" => "Fast, feature-rich, GPU based terminal"
    )
  end

  conf_d = "/home/#{info["user"]}/.config/"
  kitty_d = "#{conf_d}/kitty"
  kitty_conf = "#{kitty_d}/kitty.conf"
  [conf_d, kitty_d].each do |d|
    directory d do
      owner info["user"]
      group info["group"]
      mode 0o750
    end
  end
  next if info["user"]["skip_config"]

  font = info["font"] || "Ubuntu Mono"
  font_size = info["font_size"] || "13"
  file kitty_conf do
    owner info["user"]
    group info["group"]
    mode 0o640
    content <<~EOH
      copy_on_select yes
      map cmd+c        copy_to_clipboard
      map cmd+v        paste_from_clipboard
      map shift+insert paste_from_clipboard
      enable_audio_bell no

      font_family #{font}
      font_size #{font_size}

      # tango light
      cursor #000000
      cursor_text_color background
      foreground #2E3436
      background #EEEEEC
      selection_foreground #ffffff
      selection_background #000000
      color0 #2E3436
      color8 #555753
      color1 #CC0000
      color9 #EF2929
      color2  #4E9A06
      color10 #8AE234
      color3  #C4A000
      color11 #FCE94F
      color4  #3465A4
      color12 #729FCF
      color5  #75507B
      color13 #AD7FA8
      color6  #06989A
      color14 #34E2E2
      color7  #D3D7CF
      color15 #D3D7CF
    EOH
  end
end
