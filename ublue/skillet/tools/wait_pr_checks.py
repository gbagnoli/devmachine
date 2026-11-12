"""Wait for required checks to register before delegating readiness to gh."""

import argparse
import subprocess
import time


def wait_for_checks(repository, pull_request, attempts=60, interval=10):
    for attempt in range(attempts):
        result = subprocess.run(
            ["gh", "pr", "checks", pull_request, "--required", "--repo", repository],
            capture_output=True,
            text=True,
            check=False,
        )
        if "no checks reported" not in result.stderr.lower():
            # gh distinguishes pending (8), failed (1), and passed (0).
            # Delegate actual readiness and failure handling to its watcher.
            if result.returncode not in (0, 1, 8):
                raise RuntimeError(
                    f"Checking PR status failed: {result.stderr.strip()}"
                )
            if not result.stdout.strip():
                raise RuntimeError(f"No usable check status: {result.stderr.strip()}")
            return
        if attempt + 1 < attempts:
            print("Waiting for required PR checks to register", flush=True)
            time.sleep(interval)
    raise TimeoutError("Required PR checks did not register within the bounded wait")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--pr", required=True)
    args = parser.parse_args()
    wait_for_checks(args.repo, args.pr)
    subprocess.run(
        [
            "gh",
            "pr",
            "checks",
            args.pr,
            "--required",
            "--watch",
            "--fail-fast",
            "--repo",
            args.repo,
        ],
        check=True,
    )


if __name__ == "__main__":
    main()
