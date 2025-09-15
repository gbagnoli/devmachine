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
* [cinc-workstation](https://cinc.sh/start/workstation/)
* [rbenv](https://github.com/rbenv/rbenv)
* [rbenv-cinc-workstation](https://github.com/yacn/rbenv-cinc-workstation.git)
* [brew](https://brew.sh)
* [uv](https://github.com/astral-sh/uv)

Once installed cinc-workstation from deb:

```bash
sudo apt install rbenv git
rbenv init
# you can add the init eval to the ~/.bashrc or ~/.bash_profile
# now, install ruby-build
mkdir -p "$(rbenv root)"/plugins
git clone https://github.com/rbenv/ruby-build.git "$(rbenv root)"/plugins/ruby-build
# install rbenv-chefdk
git clone https://github.com/yacn/rbenv-cinc-workstation.git  "$(rbenv root)"/plugins/rbenv-cinc-workstation
mkdir "$(rbenv root)/versions/cinc-workstation"
rbenv shell cinc-workstation
rbenv rehash
# install brew
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
export PATH="$PATH:/home/linuxbrew/.linuxbrew/bin"
brew install uv
uv venv
source .venv/bin/activate
uv sync --dev
# install the git commit hook that will run all needed linters
ln -s $(pwd)/hooks/pre-commit.sh .git/hooks/pre-commit
ln -s $(pwd)/hooks/pre-push.sh .git/hooks/pre-push
```

In case you use [autoenv](https://github.com/kennethreitz/autoenv) you can add this to the `.env` file

```bash
#!/bin/bash

if [ -z ${VIRTUAL_ENV+x} ] && [ -f .venv/bin/activate ]; then
  .  .venv/bin/activate
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

## kitchen tests with vagrant

Install [vagrant](https://www.vagrantup.com/downloads.html) using the deb from
the site and virtualbox (`sudo apt install virtualbox`) then:

```bash
chef exec kitchen test ubik-ubuntu-1804
```

You can list the available tests with

```bash
chef exec kitchen list
```
