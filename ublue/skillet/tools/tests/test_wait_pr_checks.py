"""Check registration is eventually consistent, but failures must not be hidden."""

import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import wait_pr_checks  # noqa: E402


class RegistrationTests(unittest.TestCase):
    @patch("wait_pr_checks.time.sleep")
    @patch("wait_pr_checks.subprocess.run")
    def test_waits_for_registration_then_accepts_pending(self, run, sleep):
        run.side_effect = [
            subprocess.CompletedProcess([], 1, "", "no checks reported on branch"),
            subprocess.CompletedProcess([], 8, "image-check pending", ""),
        ]
        wait_pr_checks.wait_for_checks("owner/repo", "12", attempts=2)
        self.assertEqual(run.call_count, 2)
        sleep.assert_called_once_with(10)

    @patch("wait_pr_checks.subprocess.run")
    def test_authentication_errors_do_not_retry(self, run):
        run.return_value = subprocess.CompletedProcess([], 1, "", "HTTP 401")
        with self.assertRaises(RuntimeError):
            wait_pr_checks.wait_for_checks("owner/repo", "12")
        run.assert_called_once()

    @patch("wait_pr_checks.time.sleep")
    @patch("wait_pr_checks.subprocess.run")
    def test_missing_checks_timeout_without_merge(self, run, sleep):
        run.return_value = subprocess.CompletedProcess([], 1, "", "no checks reported")
        with self.assertRaises(TimeoutError):
            wait_pr_checks.wait_for_checks("owner/repo", "12", attempts=2)
        self.assertEqual(run.call_count, 2)
        sleep.assert_called_once()
