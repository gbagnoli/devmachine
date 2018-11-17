#!/usr/bin/env python
from __future__ import print_function
import os
import socket
import sys
from io import StringIO
from fabric.api import (
    env,
    hide,
    local,
    put,
    task,
    settings,
    sudo
)
from fabric.contrib.project import rsync_project
from fabric.contrib.files import exists as remote_exists
from fabric.contrib.files import contains as remote_contains

env.use_ssh_config = True
chef_command = "chef-client -N {0} -z -c chef-client.rb -o 'role[{0}]"
chef_script = '''
#!/bin/bash
cd {remote}
chef-client -N {host} -z -c chef-client.rb -o 'role[{host}]' "$@"
'''
script = '/usr/local/bin/run-chef'
wrapper_script = '''#!/bin/bash
sudo {} "$@"
'''.format(script)
sudoers = '/etc/sudoers.d/chef'


def vendor() -> None:
    with settings(hide('stdout')):
        local("bundle exec berks vendor")


def chef(host: str, remote: str) -> None:
    cmd = chef_command.format(host)
    wrapper = '/usr/local/bin/run-chef-{}'.format(env.user)
    if not remote_exists(script) or not remote_contains(script, cmd):
        put(StringIO(chef_script.format(remote=remote, host=host)),
            script, use_sudo=True, mode='0750')
    if remote_exists(sudoers):
        sudo("rm -f {}".format(sudoers))
    if not remote_exists(wrapper):
        put(StringIO(wrapper_script), wrapper, mode='0750', use_sudo=True)

    sudo(script)


def local_chef(localhost: str) -> None:
    wrapper = '/usr/local/bin/run-chef-{}'.format(env.user)
    if not os.path.exists(script):
        tmp = os.path.join('/tmp', os.path.basename(script))
        with open(tmp, 'w') as f:
            f.write(chef_script.format(remote=os.getcwd(), host=localhost))
        local('sudo install -T -m 755 {} {}'.format(
            tmp, script))
        os.unlink(tmp)
    local("sudo rm -f {}".format(sudoers))
    if not os.path.exists(wrapper):
        tmp = os.path.join('/tmp', os.path.basename(wrapper))
        with open(tmp, 'w') as f:
            f.write(wrapper_script)
        local('sudo install -T -m 755 -o {} {} {}'.format(env.user, tmp,
              wrapper))
        os.unlink(tmp)
    local(wrapper)


def rsync(remote: str) -> None:
    if not remote_exists(remote):
        sudo("mkdir -p {}".format(remote))
        sudo("chown {} {}".format(env.user, remote))

    rsync_project(local_dir="./",
                  remote_dir=remote,
                  exclude=("data", "boostrap", "local-mode-cache", ".git"),
                  extra_opts="-q",
                  delete=True)


def install_git_hooks(here: str) -> None:
    print("Installing git hooks")
    pre_commit_src = os.path.join(here, 'hooks', 'pre-commit.sh')
    pre_push_src = os.path.join(here, 'hooks', 'pre-push.sh')
    pre_commit = os.path.join(here, '.git', 'hooks', 'pre-commit')
    pre_push = os.path.join(here, '.git', 'hooks', 'pre-push')
    hooks = {
        pre_commit: pre_commit_src,
        pre_push: pre_push_src,
    }
    for dest, src in hooks.items():
        try:
            os.symlink(src, dest)
            print(" - {} -> {}".format(dest, src))
        except OSError as e:
            if e.errno == 17:
                continue
            raise e


@task
def run(remote: str="/usr/local/src/chefrepo/") -> None:
    here = os.path.dirname(os.path.abspath(__file__))
    install_git_hooks(here)
    if env.host_string is None:
        vendor()
        local_chef(socket.gethostname())
    elif env.host_string.startswith("localhost_"):
        vendor()
        local_chef(env.host_string.replace('localhost_', ''))
    else:
        host = env.host_string
        os.chdir(here)
        rolefile = os.path.join(here, "roles", "{}.rb".format(host))
        if not os.path.isfile(rolefile):
            print("Cannot find file {}, aborting".format(
                rolefile), file=sys.stderr)
            sys.exit(1)
        vendor()
        rsync(remote=remote)
        chef(env.host_string, remote)
