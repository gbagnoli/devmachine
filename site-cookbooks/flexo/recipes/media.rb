Chef::Recipe.include Flexo::RandomPassword

package 'git'
package 'unrar'

node.override['nodejs']['repo'] = 'https://deb.nodesource.com/node_8.x'
include_recipe 'nodejs::nodejs_from_package'
include_recipe 'nodejs::npm'

users = node['server']['users'].reject { |_, v| v['delete'] }.keys.dup
users << 'plex'

group 'media' do
  gid node['flexo']['media']['gid']
  members users
  append true
end

package 'acl'

media_d = node['flexo']['media']['path']

user node['flexo']['media']['username'] do
  uid node['flexo']['media']['uid']
  gid 'media'
  system true
  shell '/bin/false'
end

directory media_d do
  group 'media'
  owner node['flexo']['media']['username']
  mode '2775'
end

directory "#{media_d}/downloads" do
  group 'media'
  owner node['flexo']['media']['username']
  mode '2775'
end

execute "setfacl_#{media_d}" do
  command "setfacl -R -d -m g::rwx -m o::rx #{media_d}"
  user 'root'
  not_if "getfacl #{media_d} 2>/dev/null | grep 'default:' -q"
end

python_runtime '2.7'

virtualenv_path = '/var/lib/virtualenvs/2.7'
directory virtualenv_path do
  recursive true
  group 'media'
  mode '0775'
end

# rubocop:disable Metrics/BlockLength
{
  'sickchill' => {
    repo: 'https://github.com/SickChill/SickChill.git',
    command: '%<venv>s/bin/python %<venv>s/src/%<app>s/SickBeard.py --nolaunch '\
             '-q --datadir=%<datadir>s -p %<port>s',
    config_fname: 'config.ini',
    py_packages: [],
    dir: 'series'
  },
  'couchpotato' => {
    command: '%<venv>s/bin/python %<venv>s/src/%<app>s/CouchPotato.py'\
             ' --quiet --data_dir=%<datadir>s',
    repo: 'https://github.com/CouchPotato/CouchPotatoServer.git',
    config_fname: 'settings.conf',
    py_packages: %w[lxml pyopenssl],
    dir: 'movies'
  }
}.each do |app, config|
  venv = "#{virtualenv_path}/#{app}"
  datadir = "/var/lib/#{app}"
  root_d = "#{media_d}/#{config[:dir]}"
  download_d = "#{media_d}/downloads/#{config[:dir]}"

  directory root_d do
    group 'media'
    owner node['flexo']['media']['username']
    mode '2775'
  end

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

  config[:py_packages].each do |pkg|
    python_package pkg do
      virtualenv venv
    end
  end

  directory "#{venv}/src" do
    group 'media'
    owner node['flexo']['media']['username']
    mode '0750'
  end

  git "#{venv}/src/#{app}" do
    repository config[:repo]
    action :sync
    revision 'master'
    checkout_branch 'master'
    user node['flexo']['media']['username']
    notifies :run, "bash[install #{app}]", :immediately
    notifies :restart, "systemd_unit[#{app}.service]", :delayed
  end

  bash "install #{app}" do
    action :nothing
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
      api_key: api_key,
      download_dir: download_d,
      root_dir: root_d
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
# rubocop:enable Metrics/BlockLength

directory '/var/www/' do
  owner 'www-data'
  group 'www-data'
end

cookbook_file '/var/www/index.html' do
  source 'index.html'
  owner 'www-data'
  group 'www-data'
end

nginx_site 'media.tigc.eu' do
  template 'media.nginx.erb'
  variables(
    host: '127.0.0.1',
    sickchill_port: node['flexo']['media']['sickchill']['port'],
    couchpotato_port: node['flexo']['media']['couchpotato']['port'],
    server_name: 'media.tigc.eu',
    oauth2_proxy_port: lazy { node['server']['oauth2_proxy']['http_port'] },
    oauth2_proxy_upstream_port: lazy { node['server']['oauth2_proxy']['upstream_port'] }
  )
  action :enable
end
