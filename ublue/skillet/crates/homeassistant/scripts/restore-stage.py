"""Stage Home Assistant's documented restore instruction using its guest credential."""

import json
import os
import subprocess
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: stage.py CONFIG_DIR ARCHIVE_IN_CONFIG_DIR")
    config = Path(sys.argv[1])
    archive = Path(sys.argv[2])
    if not config.is_dir() or config.is_symlink() or not archive.is_file():
        raise SystemExit("missing or unsafe restore directory/archive")
    if archive.parent != config / "backups" or archive.is_symlink():
        raise SystemExit("archive must be a regular file in config/backups")
    result = subprocess.run(
        [
            "systemd-creds",
            "decrypt",
            "--name=homeassistant_backup",
            "/etc/credstore.encrypted/skillet/homeassistant_backup.cred",
            "-",
        ],
        check=True,
        capture_output=True,
    )
    settings = json.loads(result.stdout)
    host = Path("/etc/skillet/host").read_text().strip()
    if settings.get("host") != host or not settings.get("backup_password"):
        raise SystemExit("credential host or password mismatch")
    instruction = {
        "path": "/config/backups/" + archive.name,
        "password": settings["backup_password"],
        "remove_after_restore": False,
        "restore_database": True,
        "restore_homeassistant": True,
    }
    path = config / ".HA_RESTORE"
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as output:
        json.dump(instruction, output)
        output.flush()
        os.fsync(output.fileno())
    print("Home Assistant restore instruction staged")


if __name__ == "__main__":
    main()
