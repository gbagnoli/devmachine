Chef::Recipe.send(:include, Marvin::RandomPassword)

virtualenv_path = '/var/lib/virtualenvs/2.7'
sickrage_venv = "#{virtualenv_path}/sickrage"
sickrage_datadir = '/var/lib/sickrage'

package 'git'

group 'media' do
  gid node['marvin']['media']['gid']
  members node['user']['login']
  append true
end

user 'sickrage' do
  uid node['marvin']['media']['sickrage']['uid']
  gid 'media'
  system true
  shell '/bin/false'
  home sickrage_datadir
end

python_runtime '2.7'

directory virtualenv_path do
  recursive true
  group 'media'
  mode '0775'
end

python_virtualenv sickrage_venv do
  group 'media'
  user 'sickrage'
  python '2.7'
end

directory "#{sickrage_venv}/src" do
  group 'media'
  owner 'sickrage'
  mode '0750'
end

git "#{sickrage_venv}/src/sickrage" do
  repository 'https://github.com/SickRage/SickRage.git'
  action :sync
  user 'sickrage'
  notifies :run, 'bash[install sickrage]', :immediately
  notifies :restart, 'systemd_unit[sickrage.service]', :delayed
end

bash 'install sickrage' do
  action :run
  cwd sickrage_venv
  code <<-EOH
    usermod -s /bin/bash sickrage
    sudo -i -u sickrage #{sickrage_venv}/bin/pip install -e #{sickrage_venv}/src/sickrage
    usermod -s /bin/false sickrage
  EOH
end

directory sickrage_datadir do
  owner 'sickrage'
  group 'media'
  mode '0750'
  recursive true
end

cookie_secret = random_password
encryption_secret = random_password

template "#{sickrage_datadir}/config.ini" do
  owner 'sickrage'
  group 'media'
  mode '0640'
  source 'sickrage-config.ini.erb'
  action :create_if_missing
  variables(
    cookie_secret: cookie_secret,
    encryption_secret: encryption_secret
  )
end

systemd_unit 'sickrage.service' do
  content <<~EOU
    [Unit]
    Description=Sickrage

    [Service]
    User=sickrage
    Group=media
    ExecStart=#{sickrage_venv}/bin/python #{sickrage_venv}/src/sickrage/SickBeard.py --nolaunch -q --datadir=#{sickrage_datadir} -p #{node['marvin']['media']['sickrage']['port']}
    After=network-online.target

    [Install]
    WantedBy=multi-user.target
  EOU
  action %i[create enable start]
end
