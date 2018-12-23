# devmachine

[![CircleCI](https://circleci.com/gh/gbagnoli/devmachine.svg?style=svg)](https://circleci.com/gh/gbagnoli/devmachine)

Installs the development environment.  This is very opinionated, and personal,
so I don't think this is going to help anyone else -- but you can use the code
as inspiration.

There are no tests, but there is a kitchen config to 'test' converge is ok.

## bootstrap

There are several scripts in the [boostrap](./boostrap) folder.

## development

Dependencies:
* [ChefDK](https://downloads.chef.io/chefdk)
* [rbenv](https://github.com/rbenv/rbenv)
* [rbenv-chefdk](https://github.com/docwhat/rbenv-chefdk)
* [pyenv](https://github.com/pyenv/pyenv)
* python3.7 (`pyenv install 3.7.1`)

Once installed chefdk from deb:

```bash
sudo apt install rbenv git
rbenv init
# you can add the init eval to the ~/.bashrc or ~/.bash_profile
# now, install ruby-build
mkdir -p "$(rbenv root)"/plugins
git clone https://github.com/rbenv/ruby-build.git "$(rbenv root)"/plugins/ruby-build
# install rbenv-chefdk
git clone https://github.com/docwhat/rbenv-chefdk.git  "$(rbenv root)"/plugins/rbenv-chefdk
mkdir "$(rbenv root)/versions/chefdk"
rbenv shell chefdk
rbenv rehash
# install pyenv
git clone https://github.com/pyenv/pyenv.git ~/.pyenv
echo 'export PYENV_ROOT="$HOME/.pyenv"' >> ~/.bashrc
echo 'export PATH="$PYENV_ROOT/bin:$PATH"' >> ~/.bashrc
echo -e 'if command -v pyenv 1>/dev/null 2>&1; then\n  eval "$(pyenv init -)"\nfi' >> ~/.bashrc
exec "$SHELL"  # reload the settings
# install needed tools to build python
apt-get install -y make build-essential libssl-dev zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev wget curl llvm libncurses5-dev xz-utils tk-dev libxml2-dev libxmlsec1-dev libffi-dev
pyenv install 3.7.1
# this should be automatic if pyenv is installed correctly
# as per .python-version file when you cd into this directory
pyenv shell 3.7.1
# also, make sure pip and pipenv are installed
pip install -U pip pipenv
# install the git commit hook that will run all needed linters
ln -s hooks/pre-commit.sh .git/hooks/pre-commit
ln -s hooks/pre-push.sh .git/hooks/pre-push
pipenv shell
pipenv install
bundle install
```

In case you use [autoenv](https://github.com/kennethreitz/autoenv) you can add this to the `.env` file

```bash
#!/bin/bash

if [ -z ${PIPENV_ACTIVE+x} ]; then
  pipenv shell
fi
```

### converge a node

```
./run -H <nodename>
```

### ssh setup

If node is called 'nodename' it has to be possible to enter in the box
with a sudo-enabled account which just `ssh nodename`.

You can achieve this by using the `~/.ssh/config` file
i.e.

```
Host nodename
Hostname nodename.fully.qualified.com
username joe
```
