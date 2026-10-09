#!/usr/bin/env python3
"""Run inside a network-isolated, disposable Fedora systemd test container.

Only synthetic credentials/addresses are used. VM transport/lifecycle remains
owned by Skillet. Supply the deployed CLI path and a declared host profile.
"""

import argparse
import json
import pathlib
import shutil
import socket
import socketserver
import subprocess
import threading
import time


def run(*args, payload=None):
    return subprocess.run(
        args, input=payload, capture_output=True, check=True, timeout=45
    ).stdout


def wait_for(predicate):
    end = time.monotonic() + 30
    while time.monotonic() < end:
        try:
            if predicate():
                return
        except subprocess.CalledProcessError:
            pass
        time.sleep(0.2)
    raise AssertionError("bounded SMTP observation timed out")


def queue():
    return [json.loads(line) for line in run("postqueue", "-j").splitlines()]


class Sink(socketserver.StreamRequestHandler):
    def handle(self):
        self.wfile.write(b"220 isolated test sink\r\n")
        message = None
        while line := self.rfile.readline():
            self.server.commands.append(line.split(b" ", 1)[0].strip().upper())
            if message is not None:
                if line == b".\r\n":
                    self.server.messages.append(b"".join(message))
                    message = None
                    self.wfile.write(b"250 captured\r\n")
                else:
                    message.append(line)
                continue
            verb = line.split(b" ", 1)[0].strip().upper()
            if verb == b"DATA":
                message = []
                self.wfile.write(b"354 send data\r\n")
            elif verb == b"QUIT":
                self.wfile.write(b"221 bye\r\n")
                break
            else:
                # No STARTTLS or AUTH advertised: production must defer.
                self.wfile.write(b"250 isolated-test\r\n")


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary")
    parser.add_argument("host")
    parser.add_argument("--production-policy", action="store_true")
    args = parser.parse_args()
    if pathlib.Path("/run/systemd/container").read_text().strip() not in {
        "podman",
        "oci",
    } or {name for _, name in socket.if_nameindex()} != {"lo"}:
        raise RuntimeError(
            "native fixture requires a disposable Podman container with network=none"
        )
    wait_for(
        lambda: (
            run(
                "systemctl", "show", "multi-user.target", "-p", "ActiveState", "--value"
            ).strip()
            == b"active"
        )
    )
    # Reproduce ostree: package image /var directories need runtime initialization.
    assert (
        run(
            "systemctl", "show", "postfix.service", "-p", "ActiveState", "--value"
        ).strip()
        != b"active"
    )
    shutil.rmtree("/var/spool/postfix", ignore_errors=True)
    assert not pathlib.Path("/var/spool/postfix").exists()
    shutil.rmtree("/var/lib/postfix", ignore_errors=True)
    assert not pathlib.Path("/var/lib/postfix").exists()
    config = {"mode": "capture"}
    if args.production_policy:
        config = {
            "mode": "production",
            "host": "localhost",
            "port": 1025,
            "tls": "starttls",
            "username": "fixture-key",
            "password": "fixture-secret",
            "sender": "server@example.invalid",
        }
    credential = pathlib.Path("/etc/credstore.encrypted/skillet/smtp_config.cred")
    credential.parent.mkdir(parents=True, exist_ok=True)
    run(
        "systemd-creds",
        "encrypt",
        "--with-key=host",
        "--name=smtp_config",
        "-",
        str(credential),
        payload=json.dumps(config).encode(),
    )

    def apply():
        return run(
            "systemd-run",
            "--wait",
            "--pipe",
            "--collect",
            "-p",
            f"LoadCredentialEncrypted=smtp_config:{credential}",
            args.binary,
            "apply",
            "--host",
            args.host,
            "--phase",
            "smtp",
        )

    apply()
    assert pathlib.Path(
        run("postconf", "-h", "smtp_tls_CAfile").decode().strip()
    ).is_file(), "configured CA trust bundle must exist"
    pid = run("systemctl", "show", "postfix.service", "-p", "MainPID", "--value")
    apply()
    assert pid == run(
        "systemctl", "show", "postfix.service", "-p", "MainPID", "--value"
    )
    assert (
        pathlib.Path("/run/postfix/skillet/sasl_passwd").stat().st_mode & 0o777 == 0o640
    )
    if args.production_policy:
        assert (
            run(
                "runuser",
                "-u",
                "postfix",
                "--",
                "postmap",
                "-q",
                "[localhost]:1025",
                "texthash:/run/postfix/skillet/sasl_passwd",
            ).strip()
            == b"fixture-key:fixture-secret"
        )
    # No upstream exists yet: submission must queue durably.
    run(
        "sendmail",
        "-i",
        "-f",
        "smoke@example.invalid",
        "receiver@example.invalid",
        payload=b"From: smoke@example.invalid\nTo: receiver@example.invalid\nSubject: isolated acceptance\n\nfixture-marker\n",
    )
    wait_for(lambda: len(queue()) == 1)
    assert queue()[0]["recipients"][0]["address"] == "receiver@example.invalid"
    run("systemctl", "stop", "postfix.service")
    apply()
    assert len(queue()) == 1
    with Server(("127.0.0.1", 1025), Sink) as sink:
        sink.messages = []
        sink.commands = []
        threading.Thread(target=sink.serve_forever, daemon=True).start()
        run("postqueue", "-f")
        if args.production_policy:
            wait_for(lambda: b"EHLO" in sink.commands)
            time.sleep(2)
            assert len(queue()) == 1
            assert b"AUTH" not in sink.commands
            assert b"MAIL" not in sink.commands
            assert sink.messages == []
            assert queue()[0]["sender"] == "server@example.invalid"
        else:
            wait_for(lambda: bool(sink.messages))
            assert b"fixture-marker" in sink.messages[0]
            wait_for(lambda: queue() == [])
        sink.shutdown()
    # The prerequisite regenerates tmpfs state without workstation access.
    pathlib.Path("/run/postfix/skillet/sasl_passwd").unlink()
    run("systemctl", "restart", "skillet-smtp-prepare.service")
    assert pathlib.Path("/run/postfix/skillet/sasl_passwd").is_file()
    print(
        "PASS: native Postfix credential preparation, repeat apply, queue restart/retry, and TLS policy"
    )


if __name__ == "__main__":
    main()
