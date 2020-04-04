# frozen_string_literal: true

resource_name :gnome_desktop_file

property :user, String, required: true
property :group, [String, NilClass], default: nil
property :exec, String, required: true
property :type, String, default: "Application"
property :encoding, String, default: "UTF-8"
property :fullpath, [String, NilClass], default: nil
property :options, Hash, default: {}

action :create do
  app_dir.split("/").each do |dir|
    next if dir == home

    directory "#{home}/#{dir}" do
      owner user
      group groupname
      mode "0755"
    end
  end
  options["Encoding"] = encoding
  options["Type"] = type

  file path do
    owner user
    group groupname
    mode "0644"
    content <<~HEREDOC
              [Desktop Entry]
              #{options_string}
            HEREDOC
  end
end

action :delete do
  file path do
    action :delete
  end
end

action_class do
  def home
    Dir.home(new_resource.user)
  end

  def app_dir
    "#{home}/.local/share/applications"
  end

  def filename
    "#{new_resource.name.tr(" ", "_").downcase}.desktop"
  end

  def path
    new_resource.fullpath || "#{app_dir}/#{filename}"
  end

  def groupname
    new_resource.group || new_resource.user
  end

  def options_string
    str = ""
    new_resource.options.each do |k, v|
      str = "#{str}\n#{k.upcase}=#{v}"
    end
    str
  end
end
