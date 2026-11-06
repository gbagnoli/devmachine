"""Release validation uses independent Git fixtures, never real release tags."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release  # noqa: E402


class VersionTests(unittest.TestCase):
    def test_consumer_requires_a_matching_published_release(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "publication.json"
            expected = release.Release("0.1.0", "a" * 40)
            valid = {
                "tag_name": expected.tag,
                "draft": False,
                "prerelease": False,
                "published_at": "2026-11-06T12:00:00Z",
            }
            path.write_text(json.dumps(valid))
            release.verify_publication(path, expected)
            for change in [
                {"draft": True},
                {"published_at": None},
                {"prerelease": True},
                {"tag_name": "skillet-v0.2.0"},
            ]:
                path.write_text(json.dumps(valid | change))
                with (
                    self.subTest(change=change),
                    self.assertRaisesRegex(release.ReleaseError, "published"),
                ):
                    release.verify_publication(path, expected)

    def test_semver_and_prereleases(self):
        for version in ["0.1.0", "1.0.0", "12.30.4", "0.2.0-rc.1", "1.0.0-1a.0"]:
            self.assertEqual(release.validate_version(version), version)
        for version in [
            None,
            1,
            "",
            "v1.0.0",
            "1.0",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2.3-",
            "1.2.3-rc..1",
            "1.2.3-01",
            "1.2.3+metadata",
            "1.2.3-rc+git.x",
        ]:
            with self.subTest(version=version), self.assertRaises(release.ReleaseError):
                release.validate_version(version)

    def test_lock_schema_is_strict(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "release.json"
            expected = release.Release("0.1.0", "a" * 40)
            path.write_text(expected.lock_json())
            self.assertEqual(release.read_lock(path), expected)
            for text in [
                "[]",
                "{}",
                '{"version":"0.1.0","commit":"short"}',
                '{"version":"0.1.0","commit":3}',
                json.dumps({"version": "0.1.0", "commit": "A" * 40}),
                json.dumps({"version": "0.1.0", "commit": "a" * 40, "extra": True}),
                '{"version":"0.1.0","version":"0.1.0","commit":"' + "a" * 40 + '"}',
            ]:
                path.write_text(text)
                with self.subTest(text=text), self.assertRaises(release.ReleaseError):
                    release.read_lock(path)


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.workspace = self.root / "skillet"
        self.workspace.mkdir()
        (self.workspace / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/hosts/joey"]\n'
            '[workspace.package]\nversion = "0.1.0"\n'
        )
        self.member = self.workspace / "crates/hosts/joey/Cargo.toml"
        self.member.parent.mkdir(parents=True)
        self.member.write_text(
            '[package]\nname = "skillet-joey"\nversion.workspace = true\n'
        )
        self.git("init", "-q", "--initial-branch=main")
        self.commit()
        self.annotate("skillet-v0.1.0")

    def git(self, *arguments):
        return subprocess.run(
            [
                "git",
                "-C",
                str(self.root),
                "-c",
                "user.name=Release fixture",
                "-c",
                "user.email=release@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "tag.gpgsign=false",
                *arguments,
            ],
            check=True,
            text=True,
            capture_output=True,
            timeout=15,
        ).stdout.strip()

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-qm", "Release fixture")

    def annotate(self, tag):
        self.git("tag", "-a", tag, "-m", "Release fixture")

    def verify(self, tag="skillet-v0.1.0", lock=None):
        return release.verify_source(self.workspace, tag, lock)

    def test_exact_annotated_release_and_lock(self):
        actual = self.verify()
        self.assertEqual(actual.version, "0.1.0")
        self.assertEqual(actual.commit, self.git("rev-parse", "HEAD"))
        self.assertEqual(self.verify(lock=actual), actual)
        with self.assertRaisesRegex(release.ReleaseError, "dependency lock"):
            self.verify(lock=release.Release("0.1.0", "b" * 40))

    def test_tag_version_lightweight_and_moved_tags_are_rejected(self):
        locked = self.verify()
        with self.assertRaisesRegex(release.ReleaseError, "workspace version"):
            self.verify("skillet-v0.2.0")
        self.git("tag", "-d", "skillet-v0.1.0")
        self.git("tag", "skillet-v0.1.0")
        with self.assertRaisesRegex(release.ReleaseError, "annotated"):
            self.verify()
        self.git("tag", "-d", "skillet-v0.1.0")
        self.annotate("skillet-v0.1.0")
        (self.root / "change").write_text("new commit")
        self.commit()
        with self.assertRaisesRegex(release.ReleaseError, "checked-out commit"):
            self.verify()
        # Retag only this disposable fixture, then check out the old locked source.
        self.git("tag", "-f", "-a", "skillet-v0.1.0", "-m", "Moved fixture tag")
        self.git("checkout", "--detach", locked.commit)
        with self.assertRaisesRegex(release.ReleaseError, "checked-out commit"):
            self.verify(lock=locked)

    def test_dirty_and_untracked_source_are_rejected(self):
        untracked = self.root / "untracked"
        untracked.write_text("change")
        with self.assertRaisesRegex(release.ReleaseError, "clean"):
            self.verify()
        untracked.unlink()
        self.member.write_text(self.member.read_text() + "# tracked edit\n")
        with self.assertRaisesRegex(release.ReleaseError, "clean"):
            self.verify()

    def test_members_must_inherit_the_shared_version(self):
        self.member.write_text('[package]\nname="skillet-joey"\nversion="0.1.0"\n')
        self.commit()
        self.git("tag", "-d", "skillet-v0.1.0")
        self.annotate("skillet-v0.1.0")
        with self.assertRaisesRegex(release.ReleaseError, "inherit"):
            self.verify()

    def test_binary_versions_include_matching_source_and_clean_state(self):
        expected = self.verify()
        directory = self.root / "binary-fixture"
        directory.mkdir()

        def report(args, **_kwargs):
            binary = Path(args[0]).name
            return subprocess.CompletedProcess(
                args, 0, f"{binary} 0.1.0+git.{expected.commit[:12]}\n"
            )

        with patch.object(release.subprocess, "run", side_effect=report) as runner:
            release.verify_binaries(self.workspace, directory, expected)
            self.assertEqual(runner.call_count, 2)
        for version in [
            "0.2.0",
            f"0.1.0+git.{expected.commit[:12]}.dirty",
            "0.1.0+git.unknown",
        ]:
            with patch.object(
                release.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 0, f"skillet {version}"),
            ):
                with self.assertRaisesRegex(
                    release.ReleaseError, "clean source release"
                ):
                    release.verify_binaries(self.workspace, directory, expected)

    def test_cli_generates_lock_and_github_outputs(self):
        lock = self.root.parent / f"{self.root.name}-lock.json"
        output = self.root.parent / f"{self.root.name}-outputs"
        self.addCleanup(lock.unlink, missing_ok=True)
        self.addCleanup(output.unlink, missing_ok=True)
        args = [
            "release.py",
            "--workspace",
            str(self.workspace),
            "--tag",
            "skillet-v0.1.0",
            "--write-lock",
            str(lock),
            "--github-output",
            str(output),
        ]
        with patch.object(sys, "argv", args):
            self.assertEqual(release.main(), 0)
        actual = release.read_lock(lock)
        self.assertIn(f"commit={actual.commit}\n", output.read_text())
        self.assertIn("prerelease=false\n", output.read_text())


if __name__ == "__main__":
    unittest.main()
