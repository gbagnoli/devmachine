"""Seed synthetic phone-trial files in a manifest-validated disposable guest."""

import os
import pathlib
import pwd
import subprocess
import sys

root = pathlib.Path(sys.argv[1])
owner = pwd.getpwnam(sys.argv[2])
assert root.is_absolute() and root.resolve() == root
trial = root / "Quantum trial"
if not trial.exists():
    trial.mkdir(mode=0o755)
assert trial.is_dir() and not trial.is_symlink()
os.chown(trial, owner.pw_uid, owner.pw_gid)
pdf = sys.stdin.buffer.read(65536)
assert pdf.startswith(b"%PDF-") and pdf.endswith(b"%%EOF\n")
for name, content in [
    ("sample-multipage.pdf", pdf),
    (
        "Read me.txt",
        b"Synthetic Quantum trial files. Try preview, upload, rename and download.\n",
    ),
]:
    path = trial / name
    if path.exists() or path.is_symlink():
        assert path.is_file() and not path.is_symlink() and path.read_bytes() == content
    else:
        with path.open("xb") as output:
            output.write(content)
    os.chmod(path, 0o644)
    os.chown(path, owner.pw_uid, owner.pw_gid)
running = subprocess.check_output(
    ["podman", "ps", "--format", "{{.Names}}"], text=True
).splitlines()
assert "filebrowser" in running and "mikochi" not in running
assert not pathlib.Path("/etc/containers/systemd/mikochi.container").exists()
health = subprocess.run(
    [
        "podman",
        "exec",
        "filebrowser",
        "curl",
        "--fail",
        "--silent",
        "http://127.0.0.1:8080/health",
    ],
    capture_output=True,
    text=True,
    check=True,
)
assert '"message":"ok"' in health.stdout
print("Quantum running; Mikochi absent; synthetic two-page PDF and text fixture ready")
