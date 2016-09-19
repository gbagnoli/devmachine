#!/usr/bin/env python
from __future__ import print_function
import os
import sys
from fabric.api import (
    cd,
    env,
    hide,
    local,
    task,
    settings,
    sudo
)
from fabric.contrib.project import rsync_project
env.use_ssh_config = True


def vendor():
    with settings(hide('stdout')):
        local("berks vendor")


def chef(host, remote):
    with cd(remote):
        sudo(chef_command(host))


def chef_command(host):
    return "chef-client -N {0} -z -c chef-client.rb -o 'role[{0}]'"\
           .format(host)


def rsync(remote):
    sudo("mkdir -p {}".format(remote))
    sudo("chown {} {}".format(env.user, remote))
    rsync_project(local_dir="./",
                  remote_dir=remote,
                  exclude=("data", "boostrap", "local-mode-cache", ".git"),
                  extra_opts="-q")

@task
def run(remote="/usr/local/src/chefrepo/"):
    here = os.path.dirname(os.path.abspath(__file__))
    if env.host_string == "localhost":
        command = chef_command("ubik")
        vendor()
        local("sudo {}".format(command))
    else:
        host = env.host_string
        os.chdir(here)
        rolefile = os.path.join(here, "roles", "{}.rb".format(host))
        if not os.path.isfile(rolefile):
            print("Cannot file {}, aborting".format(rolefile), file=sys.stderr)
            sys.exit(1)
        vendor()
        rsync(remote=remote)
        chef(env.host_string, remote)
