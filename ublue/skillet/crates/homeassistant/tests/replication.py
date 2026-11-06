"""Guest assertions only; invoke via the locked VM guest_command adapter."""

import hashlib
import ipaddress
import json
import os
import subprocess
import sys
import time
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path("/var/lib/data/syncthing")
FOLDER = "skillet-ha-acceptance"
SOURCE = ROOT / "data/backups/homeassistant"
RECEIVED = ROOT / "data/ha-acceptance-received"


def command(*args: str) -> str:
    return subprocess.check_output(args, text=True)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def api(path: str, data: object = None) -> dict:
    config = ET.parse(ROOT / "config/config.xml")
    key = config.findtext("gui/apikey")
    assert key, "missing synthetic Syncthing API key"
    inspect = json.loads(command("podman", "inspect", "syncthing"))[0]
    addresses = inspect["NetworkSettings"]["Networks"].values()
    address = next(net["IPAddress"] for net in addresses if net["IPAddress"])
    request = urllib.request.Request(
        f"http://{ipaddress.ip_address(address)}:8384/rest/{path}",
        data=None if data is None else json.dumps(data).encode(),
        headers={"X-API-Key": key, "Content-Type": "application/json"},
        method="GET" if data is None else "PUT",
    )
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(request, timeout=15) as response:
        payload = response.read()
        return json.loads(payload) if payload else {}


def main() -> None:
    host = Path("/etc/skillet/host").read_text().strip()
    runtime = Path("/etc/hostname").read_text().strip()
    assert runtime.startswith(f"{host}-test-"), "requires disposable identity"
    assert command("systemd-detect-virt", "--vm").strip(), "requires VM"
    mode = sys.argv[1]
    if mode == "info":
        print(json.dumps({"deviceID": api("system/status")["myID"]}))
    elif mode == "link":
        payload = json.load(sys.stdin)
        peer = payload["peer"]
        sender = payload["sender"]
        config = api("config")
        assert not any(folder["id"] == FOLDER for folder in config["folders"])
        device = api("config/defaults/device")
        device.update(
            {
                "deviceID": peer["deviceID"],
                "name": "Disposable backup peer",
                "addresses": [
                    f"tcp://{ipaddress.ip_address(peer['address'])}:{int(peer.get('port', 22000))}"
                ],
            }
        )
        config["devices"] = [
            d for d in config["devices"] if d["deviceID"] != peer["deviceID"]
        ] + [device]
        folder = api("config/defaults/folder")
        path = SOURCE if sender else RECEIVED
        if not path.exists():
            path.mkdir(mode=0o700)
            owner = (ROOT / "data").stat()
            os.chown(path, owner.st_uid, owner.st_gid)
        folder.update(
            {
                "id": FOLDER,
                "label": "Disposable encrypted HA backups",
                "path": f"/var/syncthing/{path.relative_to(ROOT)}",
                "type": "sendonly" if sender else "receiveonly",
                "rescanIntervalS": 5,
                "devices": [
                    {"deviceID": api("system/status")["myID"]},
                    {"deviceID": peer["deviceID"]},
                ],
            }
        )
        config["folders"].append(folder)
        api("config", config)
        print("Configured disposable backup folder")
    elif mode == "unlink":
        peer = json.load(sys.stdin)
        config = api("config")
        config["folders"] = [
            folder for folder in config["folders"] if folder["id"] != FOLDER
        ]
        config["devices"] = [
            device
            for device in config["devices"]
            if device["deviceID"] != peer["deviceID"]
        ]
        api("config", config)
        print("Removed disposable peer declaration; encrypted exports retained")
    elif mode == "hashes":
        archives = {path.name: digest(path) for path in SOURCE.glob("ha-*.tar")}
        assert archives, "no encrypted exports"
        print(json.dumps(archives))
    elif mode == "received":
        expected = json.load(sys.stdin)
        for _ in range(120):
            if all(
                (RECEIVED / name).is_file() and digest(RECEIVED / name) == value
                for name, value in expected.items()
            ):
                print("Accepted: independent VM received matching encrypted archives")
                return
            time.sleep(3)
        raise AssertionError("Syncthing peer receipt timed out")
    elif mode == "snapshot":
        expected = json.load(sys.stdin)
        subprocess.run(["systemctl", "start", "skillet-btrbk.service"], check=True)
        snapshots = Path("/var/lib/data/snapshots/syncthing").glob("syncthing.*")
        assert any(
            all(
                (snap / "data/backups/homeassistant" / name).is_file()
                and digest(snap / "data/backups/homeassistant" / name) == value
                for name, value in expected.items()
            )
            for snap in snapshots
        ), "snapshot lacks matching archive"
        print("Accepted: existing Syncthing btrbk snapshot includes matching archives")
    else:
        raise ValueError("unknown assertion mode")


if __name__ == "__main__":
    main()
