"""Exercise the installed upstream release tool in disposable Git repositories.

Cargo dependency resolution is offline. Upstream cargo-release still performs
read-only crates.io name lookups even when publishing is disabled.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release  # noqa: E402

SOURCE = Path(__file__).resolve().parents[2]


def run_fixture(root: Path) -> None:
    repository = root / "repository"
    workspace = repository / "ublue/skillet"
    workspace.mkdir(parents=True)
    environment = os.environ | {
        "CARGO_HOME": str(root / "cargo-home"),
        "CARGO_TARGET_DIR": str(root / "target"),
        "CARGO_NET_OFFLINE": "true",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_COUNT": "4",
        "GIT_CONFIG_KEY_0": "user.name",
        "GIT_CONFIG_VALUE_0": "Release fixture",
        "GIT_CONFIG_KEY_1": "user.email",
        "GIT_CONFIG_VALUE_1": "release@example.invalid",
        "GIT_CONFIG_KEY_2": "commit.gpgsign",
        "GIT_CONFIG_VALUE_2": "false",
        "GIT_CONFIG_KEY_3": "tag.gpgsign",
        "GIT_CONFIG_VALUE_3": "false",
    }

    def command(*args: str, success: bool = True) -> str:
        result = subprocess.run(
            args,
            cwd=workspace,
            env=environment,
            capture_output=True,
            text=True,
            timeout=120,
        )
        if (result.returncode == 0) != success:
            raise RuntimeError(f"{args}: {result.stdout}\n{result.stderr}")
        return result.stdout.strip()

    def git(*args: str) -> str:
        return command("git", *args)

    def snapshot() -> tuple[str, str, str, bytes, bytes]:
        return (
            git("rev-parse", "HEAD"),
            git("show-ref", "--tags"),
            git("status", "--porcelain"),
            (workspace / "Cargo.toml").read_bytes(),
            (workspace / "Cargo.lock").read_bytes(),
        )

    def change() -> None:
        source = workspace / "crates/core/src/lib.rs"
        source.write_text(source.read_text() + "// Release fixture change\n")
        git("add", "--all")
        git("commit", "-qm", "feat: fixture change")

    (workspace / "tools").mkdir()
    for filename in [
        "cut_release.py",
        "release.py",
        "ci_checks.py",
    ]:
        shutil.copyfile(SOURCE / "tools" / filename, workspace / "tools" / filename)
    (workspace / "tools/ci-checks.json").write_text(
        json.dumps(
            {
                "fixture": {
                    "name": "Fixture workspace tests",
                    "run": "cd ublue/skillet && cargo test --workspace",
                }
            }
        )
    )
    shutil.copyfile(SOURCE / "release.toml", workspace / "release.toml")
    (repository / ".gitignore").write_text("__pycache__/\ntarget/\n")
    (workspace / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/core", "crates/cli"]\nresolver = "2"\n'
        '[workspace.package]\nversion = "0.1.0"\n'
    )
    for name in ["core", "cli"]:
        crate = workspace / "crates" / name
        (crate / "src").mkdir(parents=True)
        dependencies = (
            '\n[dependencies]\nfixture-core = { path = "../core" }\n'
            if name == "cli"
            else ""
        )
        (crate / "Cargo.toml").write_text(
            f'[package]\nname = "fixture-{name}"\nversion.workspace = true\nedition = "2021"\n'
            + dependencies
        )
        (crate / "src/lib.rs").write_text("// Fixture library\n")
    git("init", "-q", "--initial-branch=master", str(repository))
    command("cargo", "generate-lockfile", "--offline")
    git("add", "--all")
    git("commit", "-qm", "Initial fixture")
    git("tag", "-a", "skillet-v0.1.0", "-m", "Skillet 0.1.0")
    old_tag = git("rev-parse", "skillet-v0.1.0")
    remote = root / "origin.git"
    command("git", "init", "--bare", str(remote))
    git("remote", "add", "origin", str(remote))
    git("push", "-u", "origin", "master", "--tags")
    remote_before = git("ls-remote", "origin")
    change()
    before = snapshot()
    launcher = [sys.executable, "tools/cut_release.py"]
    command(*launcher, "patch")
    assert snapshot() == before, "dry run mutated repository"

    for level, version in [("patch", "0.1.1"), ("rc", "0.1.2-rc.1")]:
        if level == "rc":
            change()
        previous_count = int(git("rev-list", "--count", "HEAD"))
        previous_tags = set(git("tag", "--list").splitlines())
        command(*launcher, level, "--execute", "--no-confirm")
        tag = f"skillet-v{version}"
        assert int(git("rev-list", "--count", "HEAD")) == previous_count + 1
        assert set(git("tag", "--list").splitlines()) == previous_tags | {tag}
        assert git("cat-file", "-t", tag) == "tag"
        assert git("rev-parse", f"{tag}^{{commit}}") == git("rev-parse", "HEAD")
        assert (
            git("for-each-ref", "--format=%(contents:subject)", f"refs/tags/{tag}")
            == f"Skillet {version}"
        )
        assert git("log", "-1", "--format=%s") == f"chore(skillet): release {version}"
        assert not git("status", "--porcelain")
        assert git("ls-remote", "origin") == remote_before, "release pushed remote refs"
        lock = tomllib.loads((workspace / "Cargo.lock").read_text())
        assert {package["version"] for package in lock["package"]} == {version}
        assert release.verify_source(workspace, tag).version == version
        print(
            f"PASS {version}: one commit, annotated tag, inherited versions, unchanged remote"
        )

    assert git("rev-parse", "skillet-v0.1.0") == old_tag
    git("checkout", "-qb", "forbidden-branch")
    before = snapshot()
    command(*launcher, "patch", "--execute", "--no-confirm", success=False)
    assert snapshot() == before, "branch rejection mutated repository"
    print("PASS dry-run and wrong-branch refusal preserve repository state")


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="skillet-release-") as directory:
        run_fixture(Path(directory))
