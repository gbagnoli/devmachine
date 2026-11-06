#!/usr/bin/env python3
"""Destructive guest acceptance; invoke through the owned VM guest_command helper."""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import tarfile
import tempfile
import time
import uuid
from pathlib import Path

SERVICE = "gokapi.service"
BACKUP = "skillet-gokapi-backup.service"
TIMER = "skillet-gokapi-backup.timer"
BINARY = "/var/usrlocal/bin/skillet"


def run(*args, input_text=None, timeout=180):
    result = subprocess.run(
        args, input=input_text, text=True, capture_output=True, timeout=timeout
    )
    if result.returncode:
        raise RuntimeError("guest operation failed; inspect retained guest diagnostics")
    return result.stdout


def active(unit):
    return (
        subprocess.run(
            ["systemctl", "is-active", "--quiet", unit], check=False
        ).returncode
        == 0
    )


def api(path, headers):
    # Credentials and identity remain in stdin/memory, never process arguments.
    config = "url = " + json.dumps("http://127.0.0.1:53842" + path) + "\n"
    for key, value in headers.items():
        if any(c in key + value for c in "\r\n"):
            raise RuntimeError("invalid native API header")
        config += "header = " + json.dumps(f"{key}: {value}") + "\n"
    if path == "/api/user/create":
        config += 'data-binary = ""\n'
    output = run(
        "podman",
        "exec",
        "-i",
        "gokapi",
        "curl",
        "--silent",
        "--show-error",
        "--max-time",
        "10",
        "--max-filesize",
        "1048576",
        "--config",
        "-",
        "--write-out",
        "\n%{http_code}",
        input_text=config,
        timeout=20,
    )
    body, status = output.rsplit("\n", 1)
    if status != "200":
        raise RuntimeError("native API probe failed; response suppressed")
    return body


def ready(admin):
    deadline = time.monotonic() + 60
    while True:
        try:
            api("/admin", {"X-Skillet-User": admin})
            return
        except RuntimeError:
            if time.monotonic() >= deadline:
                raise
            time.sleep(1)


def create_user(admin, name):
    token = json.loads(
        api(
            "/auth/token",
            {
                "X-Skillet-User": admin,
                "permission": "PERM_MANAGE_USERS",
            },
        )
    )["key"]
    api("/api/user/create", {"apikey": token, "username": name})


def users(database):
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
        if connection.execute("PRAGMA integrity_check").fetchall() != [("ok",)]:
            raise RuntimeError("SQLite integrity check failed")
        # LastOnline is consumer-owned and changes when readiness probes log in.
        return connection.execute(
            "SELECT Id, Name, Password, Permissions, Userlevel, ResetPassword "
            "FROM Users ORDER BY Id"
        ).fetchall()


def snapshot(archive, scratch):
    with tarfile.open(archive) as saved:
        manifest = json.load(saved.extractfile("manifest.json"))
        if manifest["kind"] != "skillet-gokapi" or manifest["format"] != 1:
            raise RuntimeError("unexpected archive manifest")
        database = scratch / "snapshot.sqlite"
        database.write_bytes(saved.extractfile("state/config/gokapi.sqlite").read())
        return {
            "config": saved.extractfile("state/config/config.json").read(),
            "marker": saved.extractfile("state/.skillet-initial.json").read(),
            "users": users(database),
        }


def verify(state, expected):
    if (state / "config/config.json").read_bytes() != expected["config"]:
        raise RuntimeError("native configuration differs after restore")
    if (state / ".skillet-initial.json").read_bytes() != expected["marker"]:
        raise RuntimeError("initial identity binding differs after restore")
    if users(state / "config/gokapi.sqlite") != expected["users"]:
        raise RuntimeError("native accounts or permissions differ after restore")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True)
    parser.add_argument("--guest-hostname", required=True)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--backups", type=Path, required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise RuntimeError("root is required in the disposable guest")
    if Path("/etc/hostname").read_text().strip() != args.guest_hostname:
        raise RuntimeError("unexpected guest identity")
    if Path("/etc/skillet/host").read_text().strip() != args.host:
        raise RuntimeError("unexpected host profile")
    run("systemd-detect-virt", "--vm")
    state, backups = args.state.resolve(strict=True), args.backups.resolve(strict=True)
    inspected = json.loads(run("podman", "inspect", "gokapi"))[0]
    mounts = {item["Destination"]: item["Source"] for item in inspected["Mounts"]}
    if any(
        mounts.get(f"/app/{leaf}") != str(state / leaf) for leaf in ("config", "data")
    ):
        raise RuntimeError("state path does not match the actual Gokapi container")
    if not active(SERVICE) or not active(TIMER):
        raise RuntimeError("Gokapi and its timer must initially be active")
    if active(BACKUP):
        raise RuntimeError("a scheduled backup is already running")
    settings = json.loads((state / ".skillet-initial.json").read_text())
    admin = settings["admin"]
    guard = Path(tempfile.mkdtemp(prefix=".gokapi-acceptance-", dir=state.parent))
    os.chmod(guard, 0o700)
    original = None
    original_digest = None
    original_name = None
    restored_original = False
    run("systemctl", "stop", TIMER)
    try:
        ready(admin)
        output = run(BINARY, "gokapi", "backup")
        original = Path(output.strip().removeprefix("Gokapi backup: "))
        if original.parent != backups or not original.is_file():
            raise RuntimeError("unexpected backup destination")
        original_name = original.name
        shutil.copyfile(original, guard / "original.tar")
        os.chmod(guard / "original.tar", 0o600)
        original_digest = hashlib.sha256((guard / "original.tar").read_bytes()).digest()
        baseline = snapshot(original, guard)
        print("PASS: original state independently protected", flush=True)

        nonce = uuid.uuid4().hex
        create_user(admin, f"backup-before-{nonce}@example.invalid")
        fixture = state / "data" / f"skillet-restore-fixture-{nonce}"
        fixture.write_bytes(os.urandom(65536))
        owner = (state / "data").stat()
        os.chown(fixture, owner.st_uid, owner.st_gid)
        os.chmod(fixture, 0o600)
        fixture_bytes = fixture.read_bytes()
        previous = set(backups.glob("gokapi-*.tar"))
        run("systemctl", "start", BACKUP)
        if (
            run("systemctl", "show", BACKUP, "-p", "Result", "--value").strip()
            != "success"
        ):
            raise RuntimeError("scheduled backup service failed")
        created = set(backups.glob("gokapi-*.tar")) - previous
        if len(created) != 1 or not active(SERVICE):
            raise RuntimeError(
                "backup did not publish one archive and restart the service"
            )
        archive = created.pop()
        expected = snapshot(archive, guard)
        with tarfile.open(archive) as saved:
            if saved.extractfile("state/data/" + fixture.name).read() != fixture_bytes:
                raise RuntimeError("archive fixture bytes differ")
        print(
            "PASS: real backup unit published paired state and restarted Gokapi",
            flush=True,
        )

        ready(admin)
        create_user(admin, f"backup-after-{nonce}@example.invalid")
        run("systemctl", "stop", SERVICE)
        changed = json.loads((state / "config/config.json").read_text())
        changed["PublicName"] = "Restore acceptance changed"
        (state / "config/config.json").write_text(json.dumps(changed))
        fixture.write_bytes(b"changed after snapshot")
        run("systemctl", "start", SERVICE)
        ready(admin)
        recovery_output = run(BINARY, "gokapi", "restore", "--archive", str(archive))
        recovery = Path(
            recovery_output.strip().removeprefix("Gokapi restored; recovery set: ")
        )
        recovery_snapshot = snapshot(recovery, guard)
        if recovery_snapshot["config"] == expected["config"]:
            raise RuntimeError(
                "pre-restore recovery archive did not capture changed state"
            )
        if recovery_snapshot["users"] == expected["users"]:
            raise RuntimeError(
                "native account mutation was not captured before restore"
            )
        verify(state, expected)
        if fixture.read_bytes() != fixture_bytes or not active(SERVICE):
            raise RuntimeError("restore did not recover file bytes and active service")
        ready(admin)
        print(
            "PASS: restore recovered configuration, native accounts/roles and file bytes",
            flush=True,
        )
        print(
            "PASS: independent pre-restore recovery archive retained changed state",
            flush=True,
        )
    finally:
        try:
            if original_name is not None:
                protected = guard / "original.tar"
                if hashlib.sha256(protected.read_bytes()).digest() != original_digest:
                    raise RuntimeError("original safety archive changed")
                original = backups / original_name
                if (
                    original.exists()
                    and original.read_bytes() != protected.read_bytes()
                ):
                    raise RuntimeError("original archive path was replaced")
                if not original.exists():
                    shutil.copyfile(protected, original)
                    os.chmod(original, 0o600)
                run(BINARY, "gokapi", "restore", "--archive", str(original))
                if not active(SERVICE):
                    run("systemctl", "start", SERVICE)
                ready(admin)
                verify(state, baseline)
                if "fixture" in locals() and fixture.exists():
                    raise RuntimeError("fixture survived original-state cleanup")
                restored_original = True
                print(
                    "PASS: original VM state restored; temporary accounts/files removed",
                    flush=True,
                )
        finally:
            run("systemctl", "start", TIMER)
            print(f"Safety archive retained at {guard}/original.tar", flush=True)
    if not restored_original or not active(TIMER):
        raise RuntimeError("original state or scheduled backup timer was not restored")
    print("PASS: Gokapi backup/restore acceptance complete", flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Native responses/account values must not leak through exception text.
        raise SystemExit(
            f"Gokapi acceptance failed ({type(error).__name__}); inspect guest state"
        ) from None
