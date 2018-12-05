Chef::Recipe.send(:include, Flexo::RandomPassword)

package 'git'
package 'unrar'

group 'media' do
  gid node['flexo']['media']['gid']
  members node['bender']['users'].keys.sort
  append true
end

user node['flexo']['media']['username'] do
  uid node['flexo']['media']['uid']
  gid node['flexo']['media']['gid']
  system true
  shell '/bin/false'
end

python_runtime '2.7'

virtualenv_path = '/var/lib/virtualenvs/2.7'
directory virtualenv_path do
  recursive true
  group 'media'
  mode '0775'
end

{
  'sickchill' => {
    repo: 'https://github.com/SickChill/SickChill.git',
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
  attrs = node['flexo']['media'][app] || {}
  command = config[:command] % { # rubocop: disable Style/FormatString
    venv: venv,
    app: app,
    datadir: datadir,
    port: attrs['port']
  }

  # once poise-python 1.7.1 is release we can use
  # pip versions >= 18.1
  python_virtualenv venv do
    pip_version '18.0'
    group 'media'
    user node['flexo']['media']['username']
    python '2.7'
  end

  directory "#{venv}/src" do
    group 'media'
    owner node['flexo']['media']['username']
    mode '0750'
  end

  git "#{venv}/src/#{app}" do
    repository config[:repo]
    action :sync
    user node['flexo']['media']['username']
    notifies :run, "bash[install #{app}]", :immediately
    notifies :restart, "systemd_unit[#{app}.service]", :delayed
  end

  bash "install #{app}" do
    action :run
    cwd venv
    code <<-EOH
      usermod -s /bin/bash #{node['flexo']['media']['username']}
      sudo -i -u #{node['flexo']['media']['username']} #{venv}/bin/pip install -e #{venv}/src/#{app}
      usermod -s /bin/false #{node['flexo']['media']['username']}
    EOH
  end

  directory datadir do
    owner node['flexo']['media']['username']
    group 'media'
    mode '0750'
    recursive true
  end

  cookie_secret = random_password
  encryption_secret = random_password
  api_key = random_password

  template "#{datadir}/#{config[:config_fname]}" do
    owner node['flexo']['media']['username']
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
      Wants=network-online.target
      After=network-online.target

      [Service]
      User=#{node['flexo']['media']['username']}
      Group=media
      ExecStart=#{command}

      [Install]
      WantedBy=multi-user.target
    EOU
    action %i[create enable start]
  end
end
