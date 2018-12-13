#!/usr/bin/env python
from __future__ import print_function

import os
import socket
import sys
from io import StringIO
from typing import Optional, Tuple

import json
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
        local("bundle exec berks vendor")


def validate_secrets(
    secrets_local: str, secrets: str, remote: str, local: bool
) -> bool:
    remote_path = os.path.join(remote, secrets)
    try:
        with open(secrets_local) as f:
            json.load(f)

        return False

    except Exception as e:
        print(f"Invalid secrets file at {secrets_local}: {e}.", file=sys.stderr)
        if not local and remote_exists(remote_path):
            print("File is not valid locally, but exists remotely.", file=sys.stderr)
            print("Using remote version.", file=sys.stderr)
            return True
        else:
            print("aborting", file=sys.stderr)
            sys.exit(1)


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
            "./ohai",
        ),
        extra_opts="-q",
        delete=True,
    )

    if secrets and not skip_secrets_upload:
        sudo(f"mkdir -p {remote}/secrets")
        put(secrets, os.path.join(remote, secrets), mode="0640", use_sudo=True)


def install_git_hooks(here: str) -> None:
    print("Installing git hooks")
    pre_commit_src = os.path.join(here, "hooks", "pre-commit.sh")
    pre_push_src = os.path.join(here, "hooks", "pre-push.sh")
    pre_commit = os.path.join(here, ".git", "hooks", "pre-commit")
    pre_push = os.path.join(here, ".git", "hooks", "pre-push")
    hooks = {pre_commit: pre_commit_src, pre_push: pre_push_src}
    for dest, src in hooks.items():
        try:
            os.symlink(src, dest)
            print(" - {} -> {}".format(dest, src))
        except OSError as e:
            if e.errno == 17:
                continue
            raise e


def check_node(host: str, remote: str, local: bool) -> Tuple[Optional[str], bool]:
    here = os.path.dirname(os.path.abspath(__file__))
    try:
        with open(os.path.join(here, "nodes.yaml")) as f:
            conf = yaml.load(f)
    except Exception as e:
        print(f"Cannot parse nodes.yaml: {e}", file=sys.stderr)
        sys.exit(1)

    if host not in conf["nodes"]:
        print(f"Cannot find host '{host}' in nodes.yaml", file=sys.stderr)
        sys.exit(1)

    if host in conf["require_secrets"]:
        secrets_file = os.path.join("secrets", f"{host}.json")
        secrets = os.path.join(here, secrets_file)
        skip_upload = validate_secrets(secrets, secrets_file, remote, local)

        return secrets_file, skip_upload

    rolefile = os.path.join(here, "roles", "{}.rb".format(host))
    if not os.path.isfile(rolefile):
        print("Cannot find file {rolefile}, aborting", file=sys.stderr)
        sys.exit(1)

    return None, False


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
