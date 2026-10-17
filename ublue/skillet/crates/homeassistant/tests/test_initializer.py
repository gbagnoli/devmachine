"""Exercise missing-only publication without requiring HA on the workstation."""

import asyncio
import importlib.util
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "initializer", Path(__file__).parents[1] / "src" / "initialize.py"
)
assert SPEC is not None and SPEC.loader is not None
INITIALIZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INITIALIZER)
NATIVE_HTTP = INITIALIZER.initialize_http


def generate(directory: str) -> bool:
    root = Path(directory)
    (root / "configuration.yaml").write_text(
        "default_config:\nautomation: !include automations.yaml\n"
    )
    (root / "automations.yaml").write_text("[]")
    return True


class InitializationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        module = types.ModuleType("homeassistant.config")
        module.__dict__["_write_default_config"] = generate
        self.patch = patch.dict(sys.modules, {"homeassistant.config": module})
        self.http_roots: list[Path] = []

        async def initialize_http(root: Path) -> None:
            self.http_roots.append(root)

        self.http_patch = patch.object(INITIALIZER, "initialize_http", initialize_http)
        self.http_patch.start()
        self.addCleanup(self.http_patch.stop)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def test_fresh_configuration_keeps_generated_includes(self) -> None:
        INITIALIZER.initialize(self.root)
        self.assertIn(
            "automation: !include", (self.root / "configuration.yaml").read_text()
        )
        self.assertEqual(self.http_roots, [self.root])
        self.assertNotIn("http:", (self.root / "configuration.yaml").read_text())
        self.assertEqual(INITIALIZER.HTTP["server_host"], ["127.0.0.1"])
        self.assertEqual((self.root / "automations.yaml").read_text(), "[]")

    def test_existing_configuration_and_partial_dependencies_are_preserved(
        self,
    ) -> None:
        (self.root / "automations.yaml").write_text("existing automations")
        INITIALIZER.initialize(self.root)
        self.assertEqual(
            (self.root / "automations.yaml").read_text(), "existing automations"
        )
        (self.root / "configuration.yaml").write_text("user settings")
        INITIALIZER.initialize(self.root)
        self.assertEqual(
            (self.root / "configuration.yaml").read_text(), "user settings"
        )
        self.assertEqual(self.http_roots, [self.root])

    def test_existing_native_http_store_is_preserved(self) -> None:
        (self.root / ".storage").mkdir()
        path = self.root / ".storage/http"
        path.write_text("existing application-managed settings")
        asyncio.run(NATIVE_HTTP(self.root))
        self.assertEqual(path.read_text(), "existing application-managed settings")

    def test_linked_http_storage_is_refused(self) -> None:
        (self.root / ".storage").symlink_to("elsewhere")
        with self.assertRaises(RuntimeError):
            asyncio.run(NATIVE_HTTP(self.root))

    def test_linked_destination_is_rejected(self) -> None:
        (self.root / "configuration.yaml").symlink_to("absent")
        with self.assertRaises(RuntimeError):
            INITIALIZER.initialize(self.root)


if __name__ == "__main__":
    unittest.main()
