#!/usr/bin/env python3
"""Assert shared/personal isolation inside an explicitly owned disposable guest.

Pass pre-update shared config/cert/key SHA-256 hashes as JSON on stdin.
"""

import argparse
import hashlib
import json
import os
import pwd
import re
import stat
import subprocess
import sys
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path


def run(*arguments):
    result = subprocess.run(arguments, capture_output=True, text=True, timeout=30)
    if result.returncode:
        raise RuntimeError("guest assertion command failed")
    return result.stdout


def inspect(name):
    return json.loads(run("podman", "inspect", name))[0]


def root(container):
    return next(
        Path(mount["Source"]).resolve()
        for mount in container["Mounts"]
        if mount["Destination"] == "/var/syncthing"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True)
    parser.add_argument("--guest-hostname", required=True)
    parser.add_argument("--owner", required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0
    assert Path("/etc/skillet/host").read_text().strip() == args.host
    assert Path("/etc/hostname").read_text().strip() == args.guest_hostname
    run("systemd-detect-virt", "--vm")
    expected = json.load(sys.stdin)
    assert set(expected) == {"config.xml", "cert.pem", "key.pem"}
    shared, personal = inspect("syncthing"), inspect("syncthing-personal")
    shared_root, personal_root = root(shared), root(personal)
    assert shared_root != personal_root
    user = pwd.getpwnam(args.owner)
    assert personal_root.is_relative_to(Path(user.pw_dir).resolve())
    assert stat.S_IMODE(personal_root.stat().st_mode) == 0o700
    for name, digest in expected.items():
        assert name in ("config.xml", "cert.pem", "key.pem")
        assert (
            hashlib.sha256((shared_root / "config" / name).read_bytes()).hexdigest()
            == digest
        )
    assert (shared_root / "config/cert.pem").read_bytes() != (
        personal_root / "config/cert.pem"
    ).read_bytes()
    personal_config = ET.parse(personal_root / "config/config.xml").getroot()
    assert (
        personal_config.find("device")
        .attrib["name"]
        .startswith(args.guest_hostname + "-personal.")
    )
    assert personal_config.findtext("gui/insecureAdminAccess") == "true"
    assert set(shared["NetworkSettings"]["Networks"]).isdisjoint(
        personal["NetworkSettings"]["Networks"]
    )
    for container, port in ((shared, "22000"), (personal, "22202")):
        ports = container["NetworkSettings"]["Ports"]
        assert not ports.get("8384/tcp")
        for protocol in ("tcp", "udp"):
            assert {value["HostPort"] for value in ports[f"22000/{protocol}"]} == {port}
    for mount in inspect("mikochi")["Mounts"]:
        source = Path(mount["Source"]).resolve()
        assert not personal_root.is_relative_to(source)
    config = Path("/etc/skillet/homer/assets/config.yml").read_text()
    files = re.search(r"(?ms)^- name: Files\n(.*?)(?=^- name:|\Z)", config)
    assert files and "  - name: syncthing-personal\n" in files.group(1)
    marker = ".skillet-owner-probe-" + uuid.uuid4().hex
    probe = personal_root / "data" / marker
    try:
        run(
            "podman",
            "exec",
            "syncthing-personal",
            "touch",
            "/var/syncthing/data/" + marker,
        )
        assert (probe.stat().st_uid, probe.stat().st_gid) == (user.pw_uid, user.pw_gid)
        run("runuser", "-u", args.owner, "--", "test", "-w", str(probe))
    finally:
        probe.unlink(missing_ok=True)
    run("systemctl", "is-active", "syncthing.service", "syncthing-personal.service")
    print(
        "PASS: independent identities/ports/backends; shared config and keys unchanged"
    )
    print(
        "PASS: personal worker files belong to the login user; shared browser excludes home"
    )
    print("PASS: personal UI is registered under Files; both services are active")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(
            f"Syncthing guest acceptance failed ({type(error).__name__})"
        ) from None
