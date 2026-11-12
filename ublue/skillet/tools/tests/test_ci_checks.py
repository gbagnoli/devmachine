"""Local preflight and CI consume one command list and stop on failure."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_checks  # noqa: E402


class CheckRunnerTests(unittest.TestCase):
    def test_failed_check_stops_before_later_commands(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "ublue/skillet"
            (workspace / "tools").mkdir(parents=True)
            checks = {key: {"name": key, "run": key} for key in ("first", "second")}
            (workspace / "tools/ci-checks.json").write_text(json.dumps(checks))
            with patch.object(
                ci_checks.subprocess,
                "run",
                side_effect=subprocess.CalledProcessError(1, "first"),
            ) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    ci_checks.run_checks(workspace=workspace)
                run.assert_called_once_with(
                    ["bash", "-e", "-o", "pipefail", "-c", "first"],
                    cwd=Path(directory),
                    check=True,
                )

    def test_ci_and_local_manifest_select_the_same_steps(self):
        import yaml

        workspace = ci_checks.WORKSPACE
        checks = json.loads((workspace / "tools/ci-checks.json").read_text())
        workflow = yaml.safe_load(
            (workspace.parents[1] / ".github/workflows/ci.yml").read_text()
        )
        commands = [
            step["run"]
            for step in workflow["jobs"]["skillet-checks"]["steps"]
            if "run" in step
        ]
        prefix = "python3 ublue/skillet/tools/ci_checks.py --step "
        selected = [
            command.removeprefix(prefix)
            for command in commands
            if command.startswith(prefix)
        ]
        self.assertEqual(selected, list(checks))
