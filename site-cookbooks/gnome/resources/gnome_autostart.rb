resource_name :gnome_autostart

property :name, String, name_property: true
property :script_content, String, required: true
property :user, String, required: true
property :group, String, default: 'users'
property :comment, String, default: ''
property :shell, String, default:'/bin/bash'

action :install do
  %w[.local .local/bin .local/logs .config .config/autostart].each do |dir|
    directory "#{home}/#{dir}" do
      owner user
      group group
      mode '0750'
    end
  end

  file scriptname do
    content <<-HEREDOC
#!#{new_resource.shell}
exec >> #{logname}
exec 2>&1
echo "[$(date)] Running '#{name}'"
#{new_resource.script_content}
echo "[$(date)] -- END"
echo
HEREDOC
    mode '0775'
    owner user
    group group
  end

  file desktopfile do
    content <<HEREDOC
[Desktop Entry]
Type=Application
Exec=#{scriptname}
Hidden=False
NoDisplay=False
X-GNOME-Autostart-enabled=true
Name=#{new_resource.name}
Comment=#{new_resource.comment}
HEREDOC
    owner user
    group group
    mode '0644'
  end
end

action :delete do
  [scriptname, logname, desktopfile].each do |f|
    file f do
      action :delete
    end
  end
end

action_class do
  def filename
    new_resource.name.tr(' ', '_').downcase
  end

  def home
    Dir.home(new_resource.user)
  end

  def scriptname
    "#{home}/.local/bin/gnome_autostart_#{filename}.sh"
  end

  def logname
    "#{home}/.local/logs/gnome_autostart_#{filename}.log"
  end

  def desktopfile
    "#{home}/.config/autostart/#{filename}.desktop"
  end
end
