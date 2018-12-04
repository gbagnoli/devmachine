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
- apt install rbenv or https://en.opensuse.org/User:Tsu2/Install_Ruby 
* [pyenv](https://github.com/pyenv/pyenv)
* python3.6 (`pyenv install 3.6.7`)
* [rbenv-chefdk](https://github.com/docwhat/rbenv-chefdk)
- mkdir mkdir -r ~/.rbenv/plugins/
- git clone https://github.com/docwhat/rbenv-chefdk.git

```bash
# this should be automatic if pyenv is installed correctly
# as per .python-version file
# pyenv shell 3.6.7
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
