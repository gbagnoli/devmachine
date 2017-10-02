apt_repository 'syncthing' do
  uri 'http://apt.syncthing.net/'
  distribution 'syncthing'
  components ['release']
  key 'https://syncthing.net/release-key.txt'
end

package 'syncthing'

node['syncthing']['users'].each do |user, conf|
  home="/home/#{user}"
  syncthing_conf_d = "#{home}/.config/syncthing"
  syncthing_conf = "#{syncthing_conf_d}/config.xml"
  unless conf.nil?
    host = conf['hostname']
    port = conf['port']
    if port.nil? || host.nil?
      raise Exception.new(
        "Missing port or hostname for user #{user} (conf: #{conf}")
    end

    # configure syncthing
    execute 'create syncthing config' do
      command "syncthing --generate #{syncthing_conf_d}"
      user user
      not_if { File.directory? syncthing_conf_d }
      notifies :run, 'bash[fix syncthing config]', :immediately
    end

    bash 'fix syncthing config' do
      action :nothing
      code <<-EOH
      sed -i -e 's/name="#{node['hostname']}"/name="#{host}"/' #{syncthing_conf}
      sed -i -e 's/<address>127.0.0.1:[0-9]*/<address>127.0.0.1:#{port}/' #{syncthing_conf}
      EOH
    end
  end

  unless node['syncthing']['skip_service']
    service "syncthing@#{user}" do
      action [:enable, :start]
      provider Chef::Provider::Service::Systemd
    end
  end
end
