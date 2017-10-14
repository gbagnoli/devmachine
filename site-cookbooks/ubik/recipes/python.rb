include_recipe 'pyenv::user'

node['pyenv']['user_installs'].each do |desc|
  desc['pythons'].each do |ver|
    pyenv_script "pyenv_install_bin_#{ver}_#{desc['user']}" do
      user desc['user']
      pyenv_version ver
      code 'pip install -U -q pip pipenv'
    end
  end
end
