#!/usr/bin/env python
from __future__ import print_function

import json
import os
import socket
import sys
from io import StringIO
from typing import Optional, Tuple

import yaml
from fabric.api import env, hide, local, put, settings, sudo, task
from fabric.contrib.files import contains as remote_contains
from fabric.contrib.files import exists as remote_exists
from fabric.contrib.project import rsync_project

env.use_ssh_config = True
chef_command = "chef-client -N {host} -z -c chef-client.rb -o 'role[{host}]'{secrets}"
chef_script = """
#!/bin/bash
cd {remote}
{chef_command} "$@"
"""
script = "/usr/local/bin/run-chef"
wrapper_script = """#!/bin/bash
sudo {} "$@"
""".format(
    script
)
sudoers = "/etc/sudoers.d/chef"


def vendor() -> None:
    with settings(hide("stdout")):
        local("berks vendor")


def chef(host: str, remote: str, secrets: Optional[str] = None) -> None:
    secrets_opts = ""
    if secrets:
        secrets_opts = f" -j {secrets}"

    cmd = chef_command.format(host=host, secrets=secrets_opts)

    wrapper = "/usr/local/bin/run-chef-{}".format(env.user)
    if not remote_exists(script) or not remote_contains(script, cmd):
        put(
            StringIO(chef_script.format(remote=remote, chef_command=cmd)),
            script,
            use_sudo=True,
            mode="0750",
        )
    if remote_exists(sudoers):
        sudo("rm -f {}".format(sudoers))
    if not remote_exists(wrapper):
        put(StringIO(wrapper_script), wrapper, mode="0750", use_sudo=True)

    sudo(script)


def local_chef(localhost: str, secrets: Optional[str] = None) -> None:
    secrets_opts = ""
    if secrets:
        secrets_opts = f" -j {secrets}"

    wrapper = "/usr/local/bin/run-chef-{}".format(env.user)
    cmd = chef_command.format(host=localhost, secrets=secrets_opts)
    if not os.path.exists(script):
        tmp = os.path.join("/tmp", os.path.basename(script))
        with open(tmp, "w") as f:
            f.write(chef_script.format(remote=os.getcwd(), chef_command=cmd))
        local("sudo install -T -m 755 {} {}".format(tmp, script))
        os.unlink(tmp)
    local("sudo rm -f {}".format(sudoers))
    if not os.path.exists(wrapper):
        tmp = os.path.join("/tmp", os.path.basename(wrapper))
        with open(tmp, "w") as f:
            f.write(wrapper_script)
        local("sudo install -T -m 755 -o {} {} {}".format(env.user, tmp, wrapper))
        os.unlink(tmp)
    local(wrapper)


def rsync(
    remote: str, secrets: Optional[str] = None, skip_secrets_upload: bool = False
) -> None:
    if not remote_exists(remote):
        sudo("mkdir -p {}".format(remote))
        sudo("chown {} {}".format(env.user, remote))

    rsync_project(
        local_dir="./",
        remote_dir=remote,
        exclude=(
            "data",
            "boostrap",
            "local-mode-cache",
            ".git",
            "nodes",
            "secrets",
            "ohai/plugins",
        ),
        extra_opts="-q",
        delete=True,
    )

    if secrets and not skip_secrets_upload:
        sudo(f"mkdir -p {remote}/secrets")
        put(secrets, os.path.join(remote, secrets), mode="0640", use_sudo=True)



@task
def run(remote: str = "/usr/local/src/chefrepo/") -> None:
    here = os.path.dirname(os.path.abspath(__file__))
    install_git_hooks(here)

    if env.host_string is None:
        host = socket.gethostname()
        local = True
    elif env.host_string.startswith("localhost_"):
        host = env.host_string.replace("localhost_", "")
        local = True
    else:
        host = env.host_string
        local = False

    secrets, skip_secrets_upload = check_node(host, remote, local)
    os.chdir(here)
    vendor()
    if local:
        local_chef(host, secrets)
    else:
        rsync(remote, secrets, skip_secrets_upload)
        chef(host, remote, secrets)
