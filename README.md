# devmachine

[![CI](https://github.com/gbagnoli/devmachine/actions/workflows/ci.yml/badge.svg)](https://github.com/gbagnoli/devmachine/actions/workflows/ci.yml)

Installs the development environment.  This is very opinionated, and personal,
so I don't think this is going to help anyone else -- but you can use the code
as inspiration.

There are no tests, but there is a kitchen config to 'test' converge is ok.

## bootstrap

There are several scripts in the [boostrap](./boostrap) folder.

## development

Dependencies:

* [cinc-workstation](https://cinc.sh/start/workstation/)
* [brew](https://brew.sh)
* [uv](https://github.com/astral-sh/uv)

Once installed cinc-workstation from deb:

```bash
# install brew
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
export PATH="$PATH:/home/linuxbrew/.linuxbrew/bin"
brew install uv
uv sync --dev
# install the git commit hook that will run all needed linters
ln -s $(pwd)/hooks/pre-commit.sh .git/hooks/pre-commit
ln -s $(pwd)/hooks/pre-push.sh .git/hooks/pre-push
```

### converge a node

```
./run -H <nodename>
```

to converge the node locally (i.e. without ssh)

```
./run -H local_<nodename>
```

to converge the node as system

```
./run system
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
cinc exec kitchen test ubik-ubuntu-1804
```

You can list the available tests with

```bash
cinc exec kitchen list
```
