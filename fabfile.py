#!/usr/bin/env python
import getpass
import json
import os
import socket
import sys
from io import StringIO
from typing import Optional, Tuple

import yaml
from fabric import Connection, task
from fabric.transfer import Transfer
from invoke import Context

chef_command = "chef-client --chef-license accept-silent -N {host} -z -c chef-client.rb -o 'role[{host}]'{secrets}"
chef_script = """
#!/bin/bash
cd {remote}
{chef_command} "$@"
"""
script = "/usr/local/bin/run-chef"
wrapper_script = f"""#!/bin/bash
sudo {script} "$@"
"""
sudoers = "/etc/sudoers.d/chef"


def setup_passwords(c: Connection) -> Connection:
    ssh_pass = getpass.getpass(f"SSH password for {c.original_host}: ")
    sudo_pass = getpass.getpass(f"Enter sudo password for {c.original_host}: ")

    # Mutate the connection object directly
    if ssh_pass:
        c.connect_kwargs["password"] = ssh_pass
    c.config.sudo.password = sudo_pass

    return c


def remote_exists(c: Connection, path: str) -> bool:
    return c.run(f"test -e {path}", warn=True).ok


def vendor(c: Context) -> None:
    with c.cd(os.path.dirname(os.path.abspath(__file__))):
        print("Vendoring local berks cookbooks")
        c.run("cinc exec berks vendor", hide=True)


def validate_secrets(
    c: Connection, secrets_local: str, secrets: str, remote: str, local: bool
) -> bool:
    remote_path = os.path.join(remote, secrets)
    try:
        with open(secrets_local) as f:
            json.load(f)
        return False
    except Exception as e:
        print(f"Invalid secrets file at {secrets_local}: {e}.", file=sys.stderr)
        if not local and remote_exists(c, remote_path):
            print("File is not valid locally, but exists remotely.", file=sys.stderr)
            print("Using remote version.", file=sys.stderr)
            return True
        else:
            print("Cannot find secrets neither locally nor remote.", file=sys.stderr)
            print(
                f"Check for permissions remotely at {remote_path}. Aborting",
                file=sys.stderr,
            )
            sys.exit(1)


def chef(c: Connection, host: str, remote: str, secrets: Optional[str] = None) -> None:
    secrets_opts = f" -j {secrets}" if secrets else ""
    cmd = chef_command.format(host=host, secrets=secrets_opts)
    wrapper = f"/usr/local/bin/run-chef-{c.user}"

    if (
        not remote_exists(c, script)
        or not c.run(f"grep -q '{cmd}' {script}", warn=True).ok
    ):
        Transfer(c).put(
            StringIO(chef_script.format(remote=remote, chef_command=cmd)),
            remote=script,
        )
        c.sudo(f"chmod 750 {script}")

    if remote_exists(c, sudoers):
        c.sudo(f"rm -f {sudoers}")

    if not remote_exists(c, wrapper):
        Transfer(c).put(StringIO(wrapper_script), remote=wrapper)
        c.sudo(f"chmod 750 {wrapper}")

    c.sudo(script)


def local_chef(c: Context, localhost: str, secrets: Optional[str] = None) -> None:
    secrets_opts = f" -j {secrets}" if secrets else ""
    cmd = chef_command.format(host=localhost, secrets=secrets_opts)
    wrapper = f"/usr/local/bin/run-chef-{c.user}"

    if not os.path.exists(script):
        with open("/tmp/run-chef", "w") as f:
            f.write(chef_script.format(remote=os.getcwd(), chef_command=cmd))
        c.run(f"sudo install -T -m 755 /tmp/run-chef {script}")
        os.unlink("/tmp/run-chef")

    c.run(f"sudo rm -f {sudoers}", warn=True)

    if not os.path.exists(wrapper):
        with open("/tmp/run-chef-wrapper", "w") as f:
            f.write(wrapper_script)
        c.run(f"sudo install -T -m 755 -o {c.user} /tmp/run-chef-wrapper {wrapper}")
        os.unlink("/tmp/run-chef-wrapper")

    c.run(wrapper)


def rsync(
    c: Connection,
    host: str,
    remote: str,
    secrets: Optional[str] = None,
    skip_secrets_upload: bool = False,
) -> None:
    if not remote_exists(c, remote):
        c.sudo(f"mkdir -p {remote}")
        c.sudo(f"chown {c.user} {remote}")

    excludes = [
        "data",
        "boostrap",
        "local-mode-cache",
        ".git",
        "nodes",
        "secrets",
        "ohai/plugins",
        ".gnupg",
        ".cache",
        "butane",
    ]
    exclude_opts = " ".join([f"--exclude='{e}'" for e in excludes])
    local_path = os.path.dirname(os.path.abspath(__file__))
    c.local(f"rsync -avz -q --delete {exclude_opts} {local_path}/ {host}:{remote}/")

    if secrets and not skip_secrets_upload:
        c.sudo(f"mkdir -p {remote}/secrets")
        secrets_local_path = os.path.join(local_path, secrets)
        Transfer(c).put(secrets_local_path, remote=os.path.join(remote, secrets))
        c.sudo(f"chmod 640 {os.path.join(remote, secrets)}")
    elif secrets:
        print("Copying secrets file from remote host")
        secrets_remote_path = os.path.join(remote, secrets)
        secrets_local_path = os.path.join(local_path, secrets)
        Transfer(c).get(secrets_remote_path, local=secrets_local_path)


def install_git_hooks(c: Context) -> None:
    print("Installing git hooks")
    here = os.path.dirname(os.path.abspath(__file__))
    pre_commit_src = os.path.join(here, "hooks", "pre-commit.sh")
    pre_push_src = os.path.join(here, "hooks", "pre-push.sh")
    pre_commit = os.path.join(here, ".git", "hooks", "pre-commit")
    pre_push = os.path.join(here, ".git", "hooks", "pre-push")
    hooks = {pre_commit: pre_commit_src, pre_push: pre_push_src}
    c.run(f"mkdir -p {os.path.join(here, '.git', 'hooks')}")
    for dest, src in hooks.items():
        if not os.path.lexists(dest):
            os.symlink(src, dest)
            print(f" - {dest} -> {src}")


def check_node(
    c: Connection, host: str, remote: str, local: bool
) -> Tuple[Optional[str], bool]:
    here = os.path.dirname(os.path.abspath(__file__))
    try:
        with open(os.path.join(here, "nodes.yaml")) as f:
            conf = yaml.safe_load(f)
    except Exception as e:
        print(f"Cannot parse nodes.yaml: {e}", file=sys.stderr)
        sys.exit(1)

    if host not in conf["nodes"]:
        print(f"Cannot find host '{host}' in nodes.yaml", file=sys.stderr)
        sys.exit(1)

    if host in conf["require_secrets"]:
        secrets_file = os.path.join("secrets", f"{host}.json")
        secrets = os.path.join(here, secrets_file)
        skip_upload = validate_secrets(c, secrets, secrets_file, remote, local)
        return secrets_file, skip_upload

    rolefile = os.path.join(here, "roles", f"{host}.rb")
    if not os.path.isfile(rolefile):
        print(f"Cannot find file {rolefile}, aborting", file=sys.stderr)
        sys.exit(1)

    return None, False


def resolve_host(c: Connection) -> Tuple[str, bool]:
    if c.host == "localhost":
        host = socket.gethostname()
        local = True
    elif c.host.startswith("localhost_"):
        host = c.host.replace("localhost_", "")
        local = True
    else:
        host = c.original_host
        local = False
    return host, local


@task
def sync(c: Connection, remote: str = "/usr/local/src/chefrepo/") -> None:
    here = os.path.dirname(os.path.abspath(__file__))
    ctx = Context()
    install_git_hooks(ctx)
    host, local = resolve_host(c)
    if not local:
        c = setup_passwords(c)

    secrets, skip_secrets_upload = check_node(c, host, remote, local)
    with ctx.cd(here):
        vendor(ctx)
    if not local:
        rsync(c, host, remote, secrets, skip_secrets_upload)


@task
def run(c: Connection, remote: str = "/usr/local/src/chefrepo/") -> None:
    sync(c, remote)
    here = os.path.dirname(os.path.abspath(__file__))
    ctx = Context()
    with ctx.cd(here):
        host, local = resolve_host(c)
        secrets, _ = check_node(c, host, remote, local)
        if local:
            local_chef(ctx, host, secrets)
        else:
            chef(c, host, remote, secrets)
