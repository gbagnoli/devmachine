# frozen_string_literal: true

conf = node['ubik']['python']
user = conf['user']

pyenv_install 'user' do
  user user
end

pyenv_plugin 'pyenv-virtualenv' do
  git_url 'https://github.com/pyenv/pyenv-virtualenv.git'
  user user
end

conf['versions']&.each do |version|
  pyenv_python version do
    user user
  end
end

if conf['user_global']
  pyenv_global conf['user_global'] do
    user user
  end
end

%w(pip pipenv).each do |egg|
  pyenv_pip egg do
    user user
  end
end
