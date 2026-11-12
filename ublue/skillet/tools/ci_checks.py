"""Run the same Skillet checks locally and in GitHub Actions."""

import argparse
import json
import subprocess
from pathlib import Path

WORKSPACE = Path(__file__).resolve().parents[1]


def run_checks(step=None, workspace=WORKSPACE):
    checks = json.loads((workspace / "tools/ci-checks.json").read_text())
    selected = [checks[step]] if step else checks.values()
    for check in selected:
        print(f"Checking: {check['name']}", flush=True)
        subprocess.run(
            ["bash", "-e", "-o", "pipefail", "-c", check["run"]],
            cwd=workspace.parents[1],
            check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--step")
    run_checks(parser.parse_args().step)
