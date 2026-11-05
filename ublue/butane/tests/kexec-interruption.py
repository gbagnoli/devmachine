"""Run only as root inside an owned disposable Secure Boot/TPM guest."""

import pathlib
import subprocess
import sys
import time


def run(*arguments):
    return subprocess.run(arguments, check=True, timeout=30)


def armed():
    return pathlib.Path("/sys/kernel/kexec_loaded").read_text().strip() == "1"


def wait_for(predicate):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.2)
    raise RuntimeError("timed out waiting for kernel state")


def main():
    binary = str(pathlib.Path(sys.argv[1]).resolve(strict=True))
    unit = "skillet-kexec-interruption-test.service"
    if armed():
        raise RuntimeError("refusing to alter a previously armed kernel")
    run(
        "systemd-run",
        "--collect",
        "--unit=" + unit,
        "--property=Type=exec",
        "--property=DefaultDependencies=no",
        "--property=ExecStopPost=/usr/bin/kexec -s -u",
        binary,
        "--load-and-hold",
    )
    try:
        wait_for(armed)
        if "--handoff" in sys.argv[2:]:
            run("systemctl", "kexec")
            # Shutdown must terminate this test client. A cancelled handoff
            # falls through to the same cleanup rather than leaving it armed.
            time.sleep(45)
            raise RuntimeError("handoff did not terminate the test client")
        run("systemctl", "kill", "--kill-whom=main", "--signal=KILL", unit)
        wait_for(lambda: not armed())
        print("PASS: killing the loaded worker caused supervised kernel cleanup")
    finally:
        subprocess.run(["systemctl", "stop", unit], check=False, timeout=30)
        if armed():
            run("kexec", "-s", "-u")
            raise RuntimeError("supervisor failed; emergency cleanup was required")


if __name__ == "__main__":
    main()
