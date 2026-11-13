"""Run the installed cargo-release with the workspace's local-only release policy."""

import argparse
import subprocess
import sys
from pathlib import Path

from ci_checks import run_checks
from release import ReleaseError, validate_version

WORKSPACE = Path(__file__).resolve().parents[1]


def check_tool(workspace: Path) -> None:
    result = subprocess.run(
        ["cargo", "release", "--version"],
        capture_output=True,
        text=True,
        timeout=10,
        cwd=workspace,
    )
    if result.returncode != 0:
        raise ReleaseError(f"cannot run cargo-release: {result.stderr.strip()}")


def release_level(value: str) -> str:
    if value in {"release", "patch", "minor", "major", "alpha", "beta", "rc"}:
        return value
    try:
        return validate_version(value)
    except ReleaseError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "level", type=release_level, help="bump level or explicit SemVer version"
    )
    parser.add_argument(
        "--execute", action="store_true", help="commit and tag; default is dry run"
    )
    parser.add_argument(
        "--no-confirm",
        action="store_true",
        help="cargo-release's noninteractive confirmation",
    )
    args = parser.parse_args()
    try:
        check_tool(WORKSPACE)
        if args.execute:
            run_checks(workspace=WORKSPACE)
        command = [
            "cargo",
            "release",
            args.level,
            "--workspace",
            "--isolated",
            "--config",
            str(WORKSPACE / "release.toml"),
            "--no-publish",
            "--no-push",
        ]
        if args.execute:
            command.append("--execute")
        if args.no_confirm:
            command.append("--no-confirm")
        return subprocess.run(command, cwd=WORKSPACE, check=False).returncode
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Release preparation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
