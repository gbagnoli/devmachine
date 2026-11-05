"""Failure acceptance for the installed service; owned disposable guest only.

Run as root after installing its executable and running `kexec install`.
Argument: `inhibited` or `killed`. Neither case should reboot the guest.
"""

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

UNIT = "skillet-kexec-activate.service"
INHIBITOR = "skillet-kexec-test-inhibitor.service"


def run(*args):
    subprocess.run(args, check=True, timeout=30)


def output(*args):
    return subprocess.check_output(args, timeout=30).decode().strip()


def wait_for(predicate):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.005)
    raise RuntimeError("timed out waiting for service acceptance state")


def armed():
    return Path("/sys/kernel/kexec_loaded").read_text().strip() == "1"


def main():
    mode = sys.argv[1]
    if mode not in ("inhibited", "killed"):
        raise ValueError("expected inhibited or killed")
    if armed() or Path("/run/skillet-kexec-owner").exists():
        raise RuntimeError("preexisting kernel or ownership marker")
    # A missed interception must fail under an inhibitor, never reboot.
    run(
        "systemd-run",
        "--collect",
        "--unit=" + INHIBITOR,
        "systemd-inhibit",
        "--what=shutdown",
        "--mode=block",
        "--who=skillet-kexec-test",
        "sleep",
        "120",
    )
    try:
        wait_for(
            lambda: (
                "skillet-kexec-test"
                in output("systemd-inhibit", "--list", "--no-pager")
            )
        )
        subprocess.run(["systemctl", "reset-failed", UNIT], check=False)
        run("systemctl", "start", UNIT)
        if mode == "killed":
            pid = int(output("systemctl", "show", UNIT, "-p", "MainPID", "--value"))

            def loader_started():
                children = Path(f"/proc/{pid}/task/{pid}/children").read_text().split()
                for child in children:
                    try:
                        if Path(f"/proc/{child}/comm").read_text().strip() == "kexec":
                            return True
                    except FileNotFoundError:
                        continue
                return False

            wait_for(loader_started)
            os.kill(pid, signal.SIGSTOP)
            wait_for(armed)
            os.kill(pid, signal.SIGKILL)
        wait_for(
            lambda: (
                output("systemctl", "show", UNIT, "-p", "ActiveState", "--value")
                == "failed"
            )
        )
        wait_for(lambda: not armed() and not Path("/run/skillet-kexec-owner").exists())
        print(f"PASS: {mode} service attempt cleared its owned kernel and marker")
    finally:
        subprocess.run(["systemctl", "stop", UNIT], check=False, timeout=30)
        run("systemctl", "stop", INHIBITOR)


if __name__ == "__main__":
    main()
