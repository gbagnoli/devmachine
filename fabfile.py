#!/usr/bin/env python
from __future__ import print_function
import os
import socket
import sys
from io import StringIO
from fabric.api import (
    cd,
    env,
    hide,
    local,
    put,
    task,
    settings,
    sudo
)
from fabric.api import run as run_as_user
from fabric.contrib.project import rsync_project
from fabric.contrib.files import exists as remote_exists
from fabric.contrib.files import contains as remote_contains
env.use_ssh_config = True
chef_command = "chef-client -N {0} -z -c chef-client.rb -o 'role[{0}]"
chef_script = '''
#!/bin/bash
cd {remote}
chef-client -N {host} -z -c chef-client.rb -o 'role[{host}]'
'''
sudoers_script = '''{user} ALL = NOPASSWD: {script}
'''
script = '/usr/local/bin/run-chef'
wrapper_script = '''#!/bin/bash
sudo {}
'''.format(script)
sudoers = '/etc/sudoers.d/chef'


def vendor():
    with settings(hide('stdout')):
        local("berks vendor")


def chef(host, remote):
    cmd = chef_command.format(host)
    wrapper = '/usr/local/bin/run-chef-{}'.format(env.user)
    if not remote_exists(script) or not remote_contains(script, cmd):
        put(StringIO(chef_script.format(remote=remote, host=host)),
            script, use_sudo=True, mode='0750')
    if not remote_exists(sudoers):
        put(StringIO(sudoers_script.format(user=env.user, script=script)),
            sudoers, mode='0440', use_sudo=True)
        sudo('chown root:root {}'.format(sudoers))
    if not remote_exists(wrapper):
        put(StringIO(wrapper_script), wrapper, mode='0750', use_sudo=True)

    run_as_user(wrapper)

def local_chef(localhost):
    wrapper = '/usr/local/bin/run-chef-{}'.format(env.user)
    cmd = chef_command.format(localhost)
    if not os.path.exists(script):
        tmp = os.path.join('/tmp', os.path.basename(script))
        with open(tmp, 'w') as f:
            f.write(chef_script.format(remote=os.getcwd(), host=localhost))
        local('sudo install -T -m 755 {} {}'.format(
            tmp, script))
        os.unlink(tmp)
    if not os.path.exists(sudoers):
        tmp = os.path.join('/tmp', os.path.basename(sudoers))
        with open(tmp, 'w') as f:
            f.write(sudoers_script.format(user=env.user, script=script))
        local('sudo install -T -m 440 {} {}'.format(
            tmp, sudoers))
        os.unlink(tmp)
    if not os.path.exists(wrapper):
        tmp = os.path.join('/tmp', os.path.basename(wrapper))
        with open(tmp, 'w') as f:
            f.write(wrapper_script)
        local('sudo install -T -m 755 -o {} {} {}'.format(env.user, tmp,
            wrapper))
        os.unlink(tmp)
    local(wrapper)


def rsync(remote):
    if not remote_exists(remote):
        sudo("mkdir -p {}".format(remote))
        sudo("chown {} {}".format(env.user, remote))

    rsync_project(local_dir="./",
                  remote_dir=remote,
                  exclude=("data", "boostrap", "local-mode-cache", ".git"),
                  extra_opts="-q",
                  delete=True)

def install_git_hooks(here):
    print("Installing git hooks")
    pre_commit_src = os.path.join(here, 'hooks', 'pre-commit.sh')
    pre_commit = os.path.join(here, '.git', 'hooks', 'pre-commit')
    pre_push = os.path.join(here, '.git', 'hooks', 'pre-push')
    hooks = {
        pre_commit: pre_commit_src,
        pre_push: pre_commit_src,
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
def run(remote="/usr/local/src/chefrepo/"):
    here = os.path.dirname(os.path.abspath(__file__))
    install_git_hooks(here)
    if env.host_string == None:
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
            print("Cannot find file {}, aborting".format(rolefile), file=sys.stderr)
            sys.exit(1)
        vendor()
        rsync(remote=remote)
        chef(env.host_string, remote)
