#!/usr/bin/env python3

import json
import os
import socket
from typing import Optional, Tuple

import click
import fabric
import patchwork.files
import yaml


def install_git_hooks(ctx: click.Context) -> None:
    click.echo("Installing git hooks")
    here = ctx.obj["root_d"]
    pre_commit_src = os.path.join(here, "hooks", "pre-commit.sh")
    pre_push_src = os.path.join(here, "hooks", "pre-push.sh")
    pre_commit = os.path.join(here, ".git", "hooks", "pre-commit")
    pre_push = os.path.join(here, ".git", "hooks", "pre-push")
    hooks = {pre_commit: pre_commit_src, pre_push: pre_push_src}
    for dest, src in hooks.items():
        try:
            os.symlink(src, dest)
            click.echo(" - {} -> {}".format(dest, src))

        except OSError as e:
            if e.errno == 17:
                continue

            ctx.fail(f"Error while trying to install hooks: {e}")


def validate_secrets(ctx: click.Context, secrets_file: str) -> bool:
    remote = ctx.obj["remote_dir"]
    remote_path = os.path.join(remote, secrets_file)
    secrets_local = os.path.join(ctx.obj["root_d"], secrets_file)
    try:
        with open(secrets_local) as f:
            json.load(f)

        return False

    except Exception as e:
        click.echo(f"Invalid secrets file at {secrets_local}: {e}.", err=True)
        if not ctx.obj["local"] and patchwork.files.exists(
            ctx.obj["connection"], remote_path
        ):
            click.echo(
                "Secrets file is not valid locally, but exists remotely. Using remote version",
                err=True,
            )
            return True
        else:
            ctx.fail("aborting")


def check_node(ctx: click.Context) -> Tuple[Optional[str], bool]:
    root_dir = ctx.obj["root_d"]
    host = ctx.obj["host"]
    try:
        with open(os.path.join(root_dir, "nodes.yaml")) as f:
            conf = yaml.load(f)
    except Exception as e:
        ctx.fail(f"Cannot parse nodes.yaml: {e}")

    if host not in conf["nodes"]:
        ctx.fail(f"Cannot find host '{host}' in nodes.yaml")

    if host in conf["require_secrets"]:
        secrets_file = os.path.join("secrets", f"{host}.json")
        skip_upload = validate_secrets(ctx, secrets_file)

        return secrets_file, skip_upload

    if not os.path.isfile(ctx.obj["role_f"]):
        ctx.fail(f"Cannot find file {ctx.obj['role_f']}")

    return None, False


@click.group()
@click.option("--remote-dir", default="/usr/local/src/chefrepo/")
@click.argument("host", required=True)
@click.pass_context
def cli(ctx: click.Context, host: str, remote_dir: str) -> None:
    here = os.path.realpath(os.path.dirname(__file__))
    ctx.obj["root_d"] = here
    ctx.ensure_object(dict)
    ctx.obj["local"] = False
    ctx.obj["remote_dir"] = remote_dir
    if host == "localhost":
        host = socket.gethostname()
        ctx.obj["local"] = True

    ctx.obj["host"] = host
    ctx.obj["connection"] = fabric.Connection(host)
    ctx.obj["role_f"] = os.path.join(here, "roles", "{}.rb".format(host))
    secrets_file, skip_secrets_upload = check_node(ctx)
    ctx.obj["secrets_file"] = secrets_file
    ctx.obj["skip_secrets_upload"] = skip_secrets_upload
    install_git_hooks(ctx)


# @cli.command()
# @click.pass_context
# def vendor(ctx: click.Context) -> None:
#     conn: fabric.Connection = ctx.obj['connection']
#     conn.local("bundle exec berks vendor", hide="stdout")


@cli.command()
@click.pass_context
def run(ctx: click.Context) -> None:
    pass


if __name__ == "__main__":
    cli(obj={})
