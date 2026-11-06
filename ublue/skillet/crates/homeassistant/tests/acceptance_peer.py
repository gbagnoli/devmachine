"""Optional VM-to-workstation Syncthing acceptance; requires existing Podman."""

import argparse
import hashlib
import json
import subprocess
import tempfile
import time
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path


def run(*args: str, stdin: bytes | None = None) -> bytes:
    return subprocess.check_output(
        args, input=b"" if stdin is None else stdin, timeout=600
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("helper", type=Path)
    parser.add_argument("butane", type=Path)
    parser.add_argument("host")
    parser.add_argument("instance")
    parser.add_argument("--sync-port", type=int, default=22100)
    parser.add_argument("--gui-port", type=int, default=28384)
    args = parser.parse_args()
    assert args.helper.is_absolute() and args.butane.is_absolute()
    assert 1024 < args.sync_port < 65536 and 1024 < args.gui_port < 65536
    prefix = [str(args.helper), str(args.butane), args.host, args.instance]
    script = Path(__file__).with_name("replication.py").resolve()
    run(*prefix, "--upload", str(script), "/var/tmp/skillet-ha-replication.py")

    def guest(mode: str, data: object = None) -> bytes:
        return run(
            *prefix,
            "/usr/bin/sudo",
            "-n",
            "python3",
            "/var/tmp/skillet-ha-replication.py",
            mode,
            stdin=None if data is None else json.dumps(data).encode(),
        )

    with tempfile.TemporaryDirectory(prefix="skillet-ha-peer-") as directory:
        root = Path(directory)
        (root / "data").mkdir()
        container = (
            run(
                "podman",
                "run",
                "--detach",
                "--pull=never",
                "--name",
                f"skillet-ha-peer-{uuid.uuid4().hex}",
                "--label",
                "skillet.acceptance=homeassistant",
                "--user",
                "0",
                "--entrypoint",
                "syncthing",
                "--volume",
                f"{root}:/fixture:Z",
                "--publish",
                f"127.0.0.1:{args.sync_port}:22000",
                "--publish",
                f"127.0.0.1:{args.gui_port}:8384",
                "docker.io/syncthing/syncthing:latest",
                "serve",
                "--home=/fixture/config",
                "--gui-address=0.0.0.0:8384",
                "--no-browser",
                "--no-restart",
            )
            .decode()
            .strip()
        )
        linked = False
        peer = {}
        try:
            for _ in range(60):
                if (root / "config/config.xml").is_file():
                    break
                time.sleep(1)
            key = ET.parse(root / "config/config.xml").findtext("gui/apikey")
            assert key, "missing synthetic receiver API key"
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

            def api(path: str, data: object = None) -> dict:
                request = urllib.request.Request(
                    f"http://127.0.0.1:{args.gui_port}/rest/{path}",
                    data=None if data is None else json.dumps(data).encode(),
                    headers={"X-API-Key": key, "Content-Type": "application/json"},
                    method="GET" if data is None else "PUT",
                )
                with opener.open(request, timeout=15) as response:
                    payload = response.read()
                    return json.loads(payload) if payload else {}

            source = json.loads(guest("info"))
            routes = json.loads(run(*prefix, "ip", "-j", "route", "show", "default"))
            gateway = next(route["gateway"] for route in routes if "gateway" in route)
            peer = {
                "deviceID": api("system/status")["myID"],
                "address": gateway,
                "port": args.sync_port,
            }
            config = api("config")
            device = api("config/defaults/device")
            device.update(
                {
                    "deviceID": source["deviceID"],
                    "name": "Disposable VM source",
                    "addresses": ["dynamic"],
                }
            )
            config["devices"].append(device)
            folder = api("config/defaults/folder")
            folder.update(
                {
                    "id": "skillet-ha-acceptance",
                    "label": "Disposable encrypted HA backups",
                    "path": "/fixture/data",
                    "type": "receiveonly",
                    "rescanIntervalS": 5,
                    "versioning": {"type": "simple", "params": {"keep": "7"}},
                    "devices": [
                        {"deviceID": source["deviceID"]},
                        {"deviceID": peer["deviceID"]},
                    ],
                }
            )
            config["folders"] = [folder]
            config["options"].update(
                {
                    "globalAnnounceEnabled": False,
                    "localAnnounceEnabled": False,
                    "natEnabled": False,
                }
            )
            api("config", config)
            linked = True
            guest("link", {"peer": peer, "sender": True})
            expected = json.loads(guest("hashes"))
            for _ in range(120):
                if all(
                    (root / "data" / name).is_file()
                    and hashlib.sha256((root / "data" / name).read_bytes()).hexdigest()
                    == digest
                    for name, digest in expected.items()
                ):
                    print(
                        "Accepted: separate rootless receiver received matching encrypted archives"
                    )
                    print(guest("snapshot", expected).decode().strip())
                    break
                time.sleep(3)
            else:
                raise AssertionError(
                    "Syncthing receiver did not receive matching archives"
                )
        finally:
            try:
                if linked:
                    guest("unlink", peer)
            finally:
                run("podman", "rm", "--force", container)


if __name__ == "__main__":
    main()
