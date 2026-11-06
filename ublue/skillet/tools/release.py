"""Validate the source release contract used by Skillet and image CI."""

import argparse
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


class ReleaseError(ValueError):
    """Release identity or source state is inconsistent."""


def validate_version(version: object) -> str:
    """Accept SemVer releases/prereleases; reserve build metadata for Git identity."""
    if not isinstance(version, str):
        raise ReleaseError("version must be a SemVer string")
    core, separator, prerelease = version.partition("-")
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", core):
        raise ReleaseError("version must be MAJOR.MINOR.PATCH with optional prerelease")
    if separator:
        for part in prerelease.split("."):
            if not re.fullmatch(r"[0-9A-Za-z-]+", part) or (
                part.isdigit() and len(part) > 1 and part.startswith("0")
            ):
                raise ReleaseError("invalid SemVer prerelease identifier")
    return version


@dataclass(frozen=True)
class Release:
    version: str
    commit: str

    def __post_init__(self) -> None:
        validate_version(self.version)
        if not isinstance(self.commit, str) or not re.fullmatch(
            r"[0-9a-f]{40}", self.commit
        ):
            raise ReleaseError("commit must be the full lowercase Git SHA")

    @property
    def tag(self) -> str:
        return f"skillet-v{self.version}"

    def lock_json(self) -> str:
        return (
            json.dumps({"version": self.version, "commit": self.commit}, indent=2)
            + "\n"
        )


def unique_fields(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ReleaseError(f"duplicate release lock field: {key}")
        result[key] = value
    return result


def read_lock(path: Path) -> Release:
    data = json.loads(path.read_text(), object_pairs_hook=unique_fields)
    if not isinstance(data, dict) or set(data) != {"version", "commit"}:
        raise ReleaseError("release lock must contain exactly version and commit")
    return Release(data["version"], data["commit"])


def git(workspace: Path, *arguments: str) -> str:
    return subprocess.run(
        ["git", "-C", str(workspace), *arguments],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    ).stdout.strip()


def verify_source(workspace: Path, tag: str, lock: Release | None = None) -> Release:
    document = tomllib.loads((workspace / "Cargo.toml").read_text())
    version = validate_version(document["workspace"]["package"]["version"])
    release = Release(version, git(workspace, "rev-parse", "--verify", "HEAD^{commit}"))
    if tag != release.tag:
        raise ReleaseError("tag does not match the workspace version")
    if lock is not None and lock != release:
        raise ReleaseError(
            "dependency lock differs from the checked-out source release"
        )
    reference = f"refs/tags/{release.tag}"
    if git(workspace, "cat-file", "-t", reference) != "tag":
        raise ReleaseError("release tags must be annotated")
    if (
        git(workspace, "rev-parse", "--verify", f"{reference}^{{commit}}")
        != release.commit
    ):
        raise ReleaseError("release tag differs from the checked-out commit")
    if git(workspace, "status", "--porcelain", "--untracked-files=normal"):
        raise ReleaseError("release source checkout must be clean")
    for member in document["workspace"]["members"]:
        matches = sorted(workspace.glob(member))
        if not matches:
            raise ReleaseError(f"missing workspace member: {member}")
        for path in matches:
            package = tomllib.loads((path / "Cargo.toml").read_text())["package"]
            if package.get("version") != {"workspace": True}:
                raise ReleaseError(f"{member} must inherit workspace.package.version")
    return release


def verify_binaries(workspace: Path, directory: Path, release: Release) -> None:
    document = tomllib.loads((workspace / "Cargo.toml").read_text())
    binaries = ["skillet"]
    for member in document["workspace"]["members"]:
        for path in workspace.glob(member):
            if path.parent == workspace / "crates/hosts":
                binaries.append(
                    tomllib.loads((path / "Cargo.toml").read_text())["package"]["name"]
                )
    for binary in binaries:
        reported = subprocess.run(
            [str((directory / binary).resolve()), "--version"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
        if reported != f"{binary} {release.version}+git.{release.commit[:12]}":
            raise ReleaseError(
                f"{binary} does not report the clean source release version"
            )


def verify_publication(path: Path, release: Release) -> None:
    data = json.loads(path.read_text())
    if (
        not isinstance(data, dict)
        or data.get("tag_name") != release.tag
        or data.get("draft") is not False
        or not isinstance(data.get("published_at"), str)
        or not data["published_at"]
        or data.get("prerelease") is not ("-" in release.version)
    ):
        raise ReleaseError("dependency must name the matching published GitHub release")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument(
        "--workspace", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--lock-file", type=Path)
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument(
        "--release-json", type=Path, help="GitHub REST release metadata"
    )
    parser.add_argument("--write-lock", type=Path)
    parser.add_argument("--github-output", type=Path)
    args = parser.parse_args()
    try:
        release = verify_source(
            args.workspace,
            args.tag,
            read_lock(args.lock_file) if args.lock_file else None,
        )
        if args.binary_dir:
            verify_binaries(args.workspace, args.binary_dir, release)
        if args.release_json:
            verify_publication(args.release_json, release)
        if args.write_lock:
            args.write_lock.write_text(release.lock_json())
        if args.github_output:
            with args.github_output.open("a") as output:
                output.write(
                    f"version={release.version}\ncommit={release.commit}\ntag={release.tag}\n"
                    f"prerelease={str('-' in release.version).lower()}\n"
                )
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        subprocess.SubprocessError,
    ) as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        return 1
    print(release.lock_json(), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
