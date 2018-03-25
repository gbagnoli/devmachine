Chef::Recipe.send(:include, Marvin::RandomPassword)

package 'git'

group 'media' do
  gid node['marvin']['media']['gid']
  members node['user']['login']
  append true
end

python_runtime '2.7'

virtualenv_path = '/var/lib/virtualenvs/2.7'
directory virtualenv_path do
  recursive true
  group 'media'
  mode '0775'
end

{
  'sickrage' => {
    repo: 'https://github.com/SickRage/SickRage.git',
    command: '%<venv>s/bin/python %<venv>s/src/%<app>s/SickBeard.py --nolaunch '\
             '-q --datadir=%<datadir>s -p %<port>s',
    config_fname: 'config.ini'
  },
  'couchpotato' => {
    command: '%<venv>s/bin/python %<venv>s/src/%<app>s/CouchPotato.py'\
             ' --quiet --data_dir=%<datadir>s',
    repo: 'https://github.com/CouchPotato/CouchPotatoServer.git',
    config_fname: 'settings.conf'
  }

}.each do |app, config|
  venv = "#{virtualenv_path}/#{app}"
  datadir = "/var/lib/#{app}"
  attrs = node['marvin']['media'][app]
  command = config[:command] % { # rubocop: disable Style/FormatString
    venv: venv,
    app: app,
    datadir: datadir,
    port: attrs['port']
  }

  user app do
    uid attrs['uid']
    gid 'media'
    system true
    shell '/bin/false'
    home datadir
  end

  python_virtualenv venv do
    group 'media'
    user app
    python '2.7'
  end

  directory "#{venv}/src" do
    group 'media'
    owner app
    mode '0750'
  end

  git "#{venv}/src/#{app}" do
    repository config[:repo]
    action :sync
    user app
    notifies :run, "bash[install #{app}]", :immediately
    notifies :restart, "systemd_unit[#{app}.service]", :delayed
  end

  bash "install #{app}" do
    action :run
    cwd venv
    code <<-EOH
      usermod -s /bin/bash #{app}
      sudo -i -u #{app} #{venv}/bin/pip install -e #{venv}/src/#{app}
      usermod -s /bin/false #{app}
    EOH
  end

  directory datadir do
    owner app
    group 'media'
    mode '0750'
    recursive true
  end

  cookie_secret = random_password
  encryption_secret = random_password
  api_key = random_password

  template "#{datadir}/#{config[:config_fname]}" do
    owner app
    group 'media'
    mode '0640'
    source "#{app}-config.ini.erb"
    action :create_if_missing
    variables(
      cookie_secret: cookie_secret,
      encryption_secret: encryption_secret,
      api_key: api_key
    )
  end

  systemd_unit "#{app}.service" do
    content <<~EOU
      [Unit]
      Description=#{app}

      [Service]
      User=#{app}
      Group=media
      ExecStart=#{command}
      After=network-online.target

      [Install]
      WantedBy=multi-user.target
    EOU
    action %i[create enable start]
  end
end
