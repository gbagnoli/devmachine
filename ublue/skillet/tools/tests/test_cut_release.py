"""The launcher checks tool compatibility before delegating any release mutation."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cut_release  # noqa: E402


class LauncherTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.workspace = Path(temporary.name)
        (self.workspace / "tools").mkdir()
        (self.workspace / "tools/cargo-release-version").write_text("0.25.16\n")

    def test_wrong_or_unusable_tool_is_rejected_before_release(self):
        for result in [
            subprocess.CompletedProcess([], 0, "cargo-release 0.26.0\n", ""),
            subprocess.CompletedProcess([], 1, "", "incompatible runtime"),
        ]:
            with (
                self.subTest(result=result),
                patch.object(cut_release, "WORKSPACE", self.workspace),
                patch.object(sys, "argv", ["cut_release.py", "patch", "--execute"]),
                patch.object(
                    cut_release.subprocess, "run", return_value=result
                ) as runner,
            ):
                self.assertEqual(cut_release.main(), 1)
                self.assertEqual(runner.call_count, 1)
                self.assertEqual(
                    runner.call_args.args[0], ["cargo", "release", "--version"]
                )

    def test_matching_tool_delegates_local_only_dry_run_or_execute(self):
        for flags in [[], ["--execute", "--no-confirm"]]:
            with (
                self.subTest(flags=flags),
                patch.object(cut_release, "run_checks") as checks,
                patch.object(cut_release, "WORKSPACE", self.workspace),
                patch.object(sys, "argv", ["cut_release.py", "0.2.0-rc.1", *flags]),
                patch.object(
                    cut_release.subprocess,
                    "run",
                    side_effect=[
                        subprocess.CompletedProcess(
                            [], 0, "cargo-release 0.25.16\n", ""
                        ),
                        subprocess.CompletedProcess([], 0),
                    ],
                ) as runner,
            ):
                self.assertEqual(cut_release.main(), 0)
                if "--execute" in flags:
                    checks.assert_called_once_with(workspace=self.workspace)
                else:
                    checks.assert_not_called()
                self.assertEqual(
                    runner.call_args.args[0],
                    [
                        "cargo",
                        "release",
                        "0.2.0-rc.1",
                        "--workspace",
                        "--isolated",
                        "--config",
                        str(self.workspace / "release.toml"),
                        "--no-publish",
                        "--no-push",
                        *flags,
                    ],
                )
                self.assertEqual(runner.call_args.kwargs["cwd"], self.workspace)

    def test_failed_preflight_prevents_release_mutation(self):
        with (
            patch.object(cut_release, "WORKSPACE", self.workspace),
            patch.object(sys, "argv", ["cut_release.py", "patch", "--execute"]),
            patch.object(cut_release, "check_tool"),
            patch.object(
                cut_release,
                "run_checks",
                side_effect=subprocess.CalledProcessError(1, "check"),
            ),
            patch.object(cut_release.subprocess, "run") as runner,
        ):
            self.assertEqual(cut_release.main(), 1)
            runner.assert_not_called()

    def test_policy_overrides_are_not_accepted_by_launcher(self):
        for flag in ["--push", "--publish", "--config", "--allow-branch"]:
            with (
                self.subTest(flag=flag),
                patch.object(sys, "argv", ["cut_release.py", "patch", flag]),
                patch.object(cut_release.subprocess, "run") as runner,
                self.assertRaises(SystemExit) as error,
            ):
                cut_release.main()
            self.assertEqual(error.exception.code, 2)
            runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
